use anyhow::Error;

/// Return true when an executor error is actually shell control flow.
///
/// These signals must propagate out of builtin dispatch so loop, function,
/// and shell-exit handlers can catch them.
pub(crate) fn is_flow_control_signal(error: &Error) -> bool {
    error
        .downcast_ref::<crate::builtins::break_builtin::BreakSignal>()
        .is_some()
        || error
            .downcast_ref::<crate::builtins::continue_builtin::ContinueSignal>()
            .is_some()
        || error
            .downcast_ref::<crate::builtins::return_builtin::ReturnSignal>()
            .is_some()
        || error
            .downcast_ref::<crate::builtins::exit_builtin::ExitSignal>()
            .is_some()
}
