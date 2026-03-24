//! The `rush.*` Lua API surface.
//!
//! Every function callable from Lua lives here. They are registered onto a
//! global Lua table named `rush` by [`register_rush_api`].
//!
//! # Available functions
//!
//! | Lua call                              | Description                                    |
//! |---------------------------------------|------------------------------------------------|
//! | `rush.exec(cmd)`                      | Run a shell command, return stdout as string   |
//! | `rush.exec_structured(cmd)`           | Run command, return structured data            |
//! | `rush.json_parse(str)`                | Decode JSON string to Lua table                |
//! | `rush.json_encode(val)`               | Encode Lua value to JSON string                |
//! | `rush.env.get(name)`                  | Read an environment variable                   |
//! | `rush.env.set(name, value)`           | Write an environment variable                  |
//! | `rush.cwd()`                          | Return current working directory               |
//! | `rush.register_builtin(name, spec)`   | Register a custom Lua builtin                  |
//! | `rush.register_prompt(name, fn)`      | Register a prompt segment function             |
//! | `rush.register_completion(name, fn)`  | Register a completion function                 |
//! | `rush.on(event, fn)`                  | Register a shell event hook                    |

use std::process::Command;

use mlua::{Function, Lua, Table, Value as LuaValue};

use crate::lua::bridge::lua_to_value;

/// Register the full `rush.*` API table into the Lua globals.
///
/// Called once by [`LuaRuntime::new`].
pub fn register_rush_api(lua: &Lua) -> mlua::Result<()> {
    let rush = lua.create_table()?;

    register_exec(lua, &rush)?;
    register_exec_structured(lua, &rush)?;
    register_json(lua, &rush)?;
    register_env(lua, &rush)?;
    register_cwd(lua, &rush)?;
    register_hooks(lua, &rush)?;
    register_builtins_api(lua, &rush)?;
    register_prompt_api(lua, &rush)?;
    register_completion_api(lua, &rush)?;

    lua.globals().set("rush", rush)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// exec / exec_structured
// ---------------------------------------------------------------------------

fn register_exec(lua: &Lua, rush: &Table) -> mlua::Result<()> {
    let exec = lua.create_function(|_, cmd: String| {
        let output = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
        let stdout = String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string();
        Ok(stdout)
    })?;
    rush.set("exec", exec)?;
    Ok(())
}

fn register_exec_structured(lua: &Lua, rush: &Table) -> mlua::Result<()> {
    let exec_structured = lua.create_function(|lua_ctx, cmd: String| {
        let output = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string();

        // Try to parse as JSON → rush Value → Lua table; fall back to string.
        match crate::value::Value::from_json(&stdout) {
            Ok(rush_val) => crate::lua::bridge::value_to_lua(lua_ctx, &rush_val),
            Err(_) => Ok(LuaValue::String(lua_ctx.create_string(&stdout)?)),
        }
    })?;
    rush.set("exec_structured", exec_structured)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// json_parse / json_encode
// ---------------------------------------------------------------------------

fn register_json(lua: &Lua, rush: &Table) -> mlua::Result<()> {
    let json_parse = lua.create_function(|lua_ctx, s: String| {
        let rush_val = crate::value::Value::from_json(&s)
            .map_err(|e| mlua::Error::RuntimeError(format!("json_parse: {}", e)))?;
        crate::lua::bridge::value_to_lua(lua_ctx, &rush_val)
    })?;

    let json_encode = lua.create_function(|_, val: LuaValue| {
        let rush_val = lua_to_value(val);
        Ok(rush_val.to_json())
    })?;

    rush.set("json_parse", json_parse)?;
    rush.set("json_encode", json_encode)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// env table
// ---------------------------------------------------------------------------

fn register_env(lua: &Lua, rush: &Table) -> mlua::Result<()> {
    let env = lua.create_table()?;

    let get = lua.create_function(|_, name: String| Ok(std::env::var(&name).ok()))?;

    let set = lua.create_function(|_, (name, value): (String, String)| {
        std::env::set_var(&name, &value);
        Ok(())
    })?;

    env.set("get", get)?;
    env.set("set", set)?;
    rush.set("env", env)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// cwd
// ---------------------------------------------------------------------------

fn register_cwd(lua: &Lua, rush: &Table) -> mlua::Result<()> {
    let cwd = lua.create_function(|_, ()| {
        let path = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        Ok(path)
    })?;
    rush.set("cwd", cwd)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Event hooks  (rush.on)
// ---------------------------------------------------------------------------

/// Registry key for the hook table: `{ event_name -> [fn, ...] }`.
const HOOKS_KEY: &str = "__rush_hooks__";

fn register_hooks(lua: &Lua, rush: &Table) -> mlua::Result<()> {
    let hooks_store: Table = lua.create_table()?;
    lua.set_named_registry_value(HOOKS_KEY, hooks_store)?;

    let on_fn = lua.create_function(|lua_ctx, (event, func): (String, Function)| {
        let store: Table = lua_ctx.named_registry_value(HOOKS_KEY)?;
        let list: Table = match store.get::<Table>(event.as_str()) {
            Ok(t) => t,
            Err(_) => {
                let t = lua_ctx.create_table()?;
                store.set(event.as_str(), t.clone())?;
                t
            }
        };
        list.raw_set(list.raw_len() + 1, func)?;
        Ok(())
    })?;

    rush.set("on", on_fn)?;
    Ok(())
}

pub(crate) fn hooks_key() -> &'static str {
    HOOKS_KEY
}

// ---------------------------------------------------------------------------
// register_builtin
// ---------------------------------------------------------------------------

/// Registry key for custom builtins: `{ name -> spec_table }`.
const BUILTINS_KEY: &str = "__rush_builtins__";

fn register_builtins_api(lua: &Lua, rush: &Table) -> mlua::Result<()> {
    let store: Table = lua.create_table()?;
    lua.set_named_registry_value(BUILTINS_KEY, store)?;

    let register = lua.create_function(|lua_ctx, (name, spec): (String, Table)| {
        let store: Table = lua_ctx.named_registry_value(BUILTINS_KEY)?;
        store.set(name.as_str(), spec)?;
        Ok(())
    })?;

    rush.set("register_builtin", register)?;
    Ok(())
}

pub(crate) fn builtins_key() -> &'static str {
    BUILTINS_KEY
}

// ---------------------------------------------------------------------------
// register_prompt
// ---------------------------------------------------------------------------

const PROMPT_KEY: &str = "__rush_prompts__";

fn register_prompt_api(lua: &Lua, rush: &Table) -> mlua::Result<()> {
    let store: Table = lua.create_table()?;
    lua.set_named_registry_value(PROMPT_KEY, store)?;

    let register = lua.create_function(|lua_ctx, (name, func): (String, Function)| {
        let store: Table = lua_ctx.named_registry_value(PROMPT_KEY)?;
        store.set(name.as_str(), func)?;
        Ok(())
    })?;

    rush.set("register_prompt", register)?;
    Ok(())
}

pub(crate) fn prompt_key() -> &'static str {
    PROMPT_KEY
}

// ---------------------------------------------------------------------------
// register_completion
// ---------------------------------------------------------------------------

const COMPLETION_KEY: &str = "__rush_completions__";

fn register_completion_api(lua: &Lua, rush: &Table) -> mlua::Result<()> {
    let store: Table = lua.create_table()?;
    lua.set_named_registry_value(COMPLETION_KEY, store)?;

    let register = lua.create_function(|lua_ctx, (name, func): (String, Function)| {
        let store: Table = lua_ctx.named_registry_value(COMPLETION_KEY)?;
        store.set(name.as_str(), func)?;
        Ok(())
    })?;

    rush.set("register_completion", register)?;
    Ok(())
}

pub(crate) fn completion_key() -> &'static str {
    COMPLETION_KEY
}
