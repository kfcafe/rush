mod case;
mod collect;
mod escape;
mod join;
mod length;
mod match_cmd;
mod pad;
mod repeat;
mod replace;
mod split;
mod sub;
mod trim;

use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::Result;

pub fn builtin_string(args: &[String], runtime: &mut Runtime) -> Result<ExecutionResult> {
    run_string_with_stdin(args, runtime, None)
}

pub fn builtin_string_with_stdin(
    args: &[String],
    runtime: &mut Runtime,
    stdin_data: &[u8],
) -> Result<ExecutionResult> {
    run_string_with_stdin(args, runtime, Some(stdin_data))
}

fn run_string_with_stdin(
    args: &[String],
    runtime: &mut Runtime,
    stdin: Option<&[u8]>,
) -> Result<ExecutionResult> {
    let subcommand = match args.first() {
        Some(s) => s.as_str(),
        None => {
            return Ok(ExecutionResult {
                output: Output::Text(String::new()),
                stderr: usage(),
                exit_code: 1,
                error: None,
            });
        }
    };

    let rest = &args[1..];

    match subcommand {
        "split" => split::run_split(rest, runtime, stdin, false),
        "split0" => split::run_split(rest, runtime, stdin, true),
        "join" => join::run_join(rest, runtime, stdin, false),
        "join0" => join::run_join(rest, runtime, stdin, true),
        "upper" => case::run_upper(rest, runtime, stdin),
        "lower" => case::run_lower(rest, runtime, stdin),
        "length" => length::run_length(rest, runtime, stdin),
        "sub" => sub::run_sub(rest, runtime, stdin),
        "collect" => collect::run_collect(rest, runtime, stdin),
        "match" => match_cmd::run_match(rest, runtime, stdin),
        "replace" => replace::run_replace(rest, runtime, stdin),
        "trim" => trim::run_trim(rest, runtime, stdin),
        "pad" => pad::run_pad(rest, runtime, stdin),
        "repeat" => repeat::run_repeat(rest, runtime, stdin),
        "escape" => escape::run_escape(rest, runtime, stdin),
        "unescape" => escape::run_unescape(rest, runtime, stdin),
        other => Ok(ExecutionResult {
            output: Output::Text(String::new()),
            stderr: format!("string: unknown subcommand '{}'\n{}", other, usage()),
            exit_code: 1,
            error: None,
        }),
    }
}

fn usage() -> String {
    "Usage: string <subcommand> [OPTIONS] [ARGS...]

Subcommands:
  split   [-m MAX] [-r] [-n] SEP [STRING...]  Split strings on SEP
  split0  [STRING...]                          Split on NUL bytes
  join    SEP [STRING...]                      Join strings with SEP
  join0   [STRING...]                          Join strings with NUL bytes
  upper   [STRING...]                          Convert to uppercase
  lower   [STRING...]                          Convert to lowercase
  length  [-q] [STRING...]                     Print character count
  sub     [-s START] [-l LEN | -e END] [STR]  Extract substring (1-based)
  collect [STRING...]                          Collect into single output
  match    [-r] [-e] [-i] [-v] PAT [STRING...]        Match strings against pattern
  replace  [-r] [-a] [-i] PAT REP [STRING...]         Replace pattern in strings
  trim     [-l] [-r] [-c CHARS] [STRING...]            Trim whitespace or chars
  pad      -w WIDTH [-r] [-c CHAR] [STRING...]         Pad strings to width
  repeat   -n COUNT [-m MAX] [STRING...]               Repeat strings N times
  escape   [--style=script|url] [STRING...]            Escape strings for shell/url
  unescape [--style=script|url] [STRING...]            Reverse of escape\n"
        .to_string()
}
