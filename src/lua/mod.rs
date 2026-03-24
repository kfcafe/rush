//! Lua extension runtime for rush.
//!
//! Embeds a Lua 5.4 interpreter via the `mlua` crate and exposes the
//! `rush.*` API to user scripts. Scripts live in `~/.rush/lua/` and are
//! loaded at startup in lexicographic order.
//!
//! # Quick start
//!
//! ```lua
//! -- ~/.rush/lua/myconfig.lua
//!
//! rush.register_builtin("greet", {
//!     description = "Say hello",
//!     run = function(args)
//!         return { text = "Hello, " .. (args[1] or "world") }
//!     end
//! })
//!
//! rush.on("precmd", function(exit_code, elapsed_ms)
//!     -- fires before every prompt
//! end)
//! ```

pub mod api;
pub mod bridge;

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use mlua::Lua;

use crate::value::Value;

/// A registered Lua builtin exposed to the rush command dispatcher.
#[derive(Debug, Clone)]
pub struct LuaBuiltin {
    /// Name used to invoke this builtin (e.g. `"weather"`).
    pub name: String,
    /// Human-readable description shown in `help` output.
    pub description: String,
}

/// The embedded Lua runtime.
///
/// Owns a single `mlua::Lua` instance with the full `rush.*` API registered.
/// Create once at shell startup and hold for the session.
pub struct LuaRuntime {
    lua: Lua,
}

impl LuaRuntime {
    /// Create a new runtime and register the `rush.*` API.
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        api::register_rush_api(&lua).map_err(|e| anyhow!("Lua API init: {}", e))?;
        Ok(Self { lua })
    }

    /// Load all `*.lua` files from `~/.rush/lua/` in lexicographic order.
    ///
    /// Missing directory is not an error. Files that fail are propagated.
    pub fn load_user_scripts(&self) -> Result<()> {
        let lua_dir = user_lua_dir();

        if !lua_dir.exists() {
            return Ok(());
        }

        let mut scripts: Vec<PathBuf> = std::fs::read_dir(&lua_dir)
            .with_context(|| format!("reading {}", lua_dir.display()))?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("lua") {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        scripts.sort();

        for script in scripts {
            let source = std::fs::read_to_string(&script)
                .with_context(|| format!("reading {}", script.display()))?;

            self.lua
                .load(&source)
                .set_name(script.to_string_lossy().as_ref())
                .exec()
                .map_err(|e| anyhow!("executing {}: {}", script.display(), e))?;
        }

        Ok(())
    }

    /// Fire a named shell event, calling all Lua hooks registered for it.
    ///
    /// Arguments are converted from rush `Value`s into Lua values. Hook
    /// errors are printed to stderr but do not abort the shell.
    pub fn call_hook(&self, name: &str, args: &[Value]) -> Result<()> {
        let hooks_store: mlua::Table = self
            .lua
            .named_registry_value(api::hooks_key())
            .map_err(|e| anyhow!("hook registry: {}", e))?;

        let list: mlua::Table = match hooks_store.get(name) {
            Ok(t) => t,
            Err(_) => return Ok(()),
        };

        let lua_args: Vec<mlua::Value> = args
            .iter()
            .map(|v| bridge::value_to_lua(&self.lua, v))
            .collect::<mlua::Result<_>>()
            .map_err(|e| anyhow!("converting hook args: {}", e))?;

        for pair in list.clone().pairs::<mlua::Integer, mlua::Function>() {
            let (_, func) = pair.map_err(|e| anyhow!("iterating hooks: {}", e))?;
            if let Err(e) = func.call::<()>(lua_args.clone()) {
                eprintln!("rush: Lua hook '{}' error: {}", name, e);
            }
        }

        Ok(())
    }

    /// Return all builtins registered by Lua scripts via `rush.register_builtin`.
    pub fn get_registered_builtins(&self) -> Vec<LuaBuiltin> {
        let store: mlua::Table = match self.lua.named_registry_value(api::builtins_key()) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

        let mut builtins = Vec::new();

        for pair in store.pairs::<String, mlua::Table>() {
            let Ok((name, spec)) = pair else { continue };
            let description = spec.get::<String>("description").unwrap_or_default();
            builtins.push(LuaBuiltin { name, description });
        }

        builtins
    }

    /// Execute a registered Lua builtin by name with the given arguments.
    ///
    /// Returns the result as a rush `Value`, or `Value::Null` if the builtin
    /// returns nothing.
    pub fn call_builtin(&self, name: &str, args: &[Value]) -> Result<Value> {
        let store: mlua::Table = self
            .lua
            .named_registry_value(api::builtins_key())
            .map_err(|e| anyhow!("builtin registry: {}", e))?;

        let spec: mlua::Table = store
            .get(name)
            .map_err(|_| anyhow!("unknown Lua builtin: {}", name))?;

        let run_fn: mlua::Function = spec
            .get("run")
            .map_err(|_| anyhow!("builtin '{}' has no 'run' function", name))?;

        // Pack args as a Lua array table.
        let lua_args = self
            .lua
            .create_table()
            .map_err(|e| anyhow!("creating args table: {}", e))?;

        for (idx, arg) in args.iter().enumerate() {
            let lua_val = bridge::value_to_lua(&self.lua, arg)
                .map_err(|e| anyhow!("converting arg {}: {}", idx, e))?;
            lua_args
                .raw_set(idx + 1, lua_val)
                .map_err(|e| anyhow!("setting arg {}: {}", idx, e))?;
        }

        let result: mlua::Value = run_fn
            .call(lua_args)
            .map_err(|e| anyhow!("calling builtin '{}': {}", name, e))?;

        Ok(bridge::lua_to_value(result))
    }

    /// Execute a Lua source string in this runtime.
    ///
    /// Useful for loading inline scripts and in integration tests.
    pub fn load_script(&self, source: &str) -> Result<()> {
        self.lua
            .load(source)
            .exec()
            .map_err(|e| anyhow!("Lua script error: {}", e))
    }

    /// Borrow the raw Lua state (unit tests only).
    #[cfg(test)]
    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}

/// Returns `~/.rush/lua/`.
fn user_lua_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".rush")
        .join("lua")
}

/// Initialise the Lua runtime, returning `None` on failure (non-fatal).
///
/// Used by the shell startup sequence. The shell continues without Lua
/// extensions if this fails.
pub fn init_lua() -> Option<LuaRuntime> {
    match LuaRuntime::new() {
        Ok(rt) => Some(rt),
        Err(e) => {
            eprintln!("rush: Lua runtime init failed: {}", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;

    fn make_rt() -> LuaRuntime {
        LuaRuntime::new().expect("LuaRuntime::new should not fail")
    }

    #[test]
    fn test_runtime_creates_successfully() {
        let _ = make_rt();
    }

    #[test]
    fn test_rush_global_is_table() {
        let rt = make_rt();
        let rush: mlua::Table = rt.lua().globals().get("rush").expect("rush global missing");
        // Verify exec is registered.
        let _: mlua::Function = rush.get("exec").expect("rush.exec missing");
    }

    #[test]
    fn test_register_and_retrieve_builtin() {
        let rt = make_rt();
        rt.lua()
            .load(
                r#"
                rush.register_builtin("test_cmd", {
                    description = "A test builtin",
                    run = function(args) return args[1] end
                })
            "#,
            )
            .exec()
            .expect("register failed");

        let builtins = rt.get_registered_builtins();
        let found = builtins.iter().find(|b| b.name == "test_cmd");
        assert!(found.is_some(), "test_cmd should be registered");
        assert_eq!(found.unwrap().description, "A test builtin");
    }

    #[test]
    fn test_call_builtin_returns_value() {
        let rt = make_rt();
        rt.lua()
            .load(
                r#"
                rush.register_builtin("echo_first", {
                    description = "echo first arg",
                    run = function(args) return args[1] end
                })
            "#,
            )
            .exec()
            .expect("register failed");

        let result = rt
            .call_builtin("echo_first", &[Value::String("hello".into())])
            .expect("call_builtin failed");

        assert_eq!(result, Value::String("hello".into()));
    }

    #[test]
    fn test_hook_registration_and_call() {
        let rt = make_rt();
        rt.lua()
            .load(
                r#"
                _hook_fired = false
                rush.on("precmd", function(exit_code)
                    _hook_fired = true
                end)
            "#,
            )
            .exec()
            .expect("hook register failed");

        rt.call_hook("precmd", &[Value::Int(0)])
            .expect("call_hook failed");

        let fired: bool = rt
            .lua()
            .globals()
            .get("_hook_fired")
            .expect("_hook_fired missing");
        assert!(fired, "hook should have fired");
    }

    #[test]
    fn test_get_registered_builtins_empty_by_default() {
        let rt = make_rt();
        assert!(rt.get_registered_builtins().is_empty());
    }

    #[test]
    fn test_init_lua_returns_some() {
        assert!(init_lua().is_some());
    }

    #[test]
    fn test_load_user_scripts_no_dir_is_ok() {
        // Verify that a missing ~/.rush/lua/ directory is handled gracefully.
        let rt = make_rt();
        let _ = rt.load_user_scripts();
    }
}
