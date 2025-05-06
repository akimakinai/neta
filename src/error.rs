#[macro_export]
macro_rules! bevyhow {
    ($fmt:expr $(,)?) => ({
        let error = ::bevy::ecs::error::BevyError::from(::std::format!($fmt));
        error
    });
    ($fmt:expr, $($arg:tt)*) => {
        ::bevy::ecs::error::BevyError::from(::std::format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! bail {
    ($fmt:expr $(,)?) => {
        return Err($crate::bevyhow!($err))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err($crate::bevyhow!($fmt, $($arg)*))
    };
}
