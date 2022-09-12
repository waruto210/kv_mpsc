//! some util macro

/// unwrap ok value in result or expr
#[macro_export]
macro_rules! unwrap_ok_or {
    ($res: expr, $e: pat, $code: expr) => {
        match $res {
            Ok(v) => v,
            Err($e) => $code,
        }
    };
}

/// unwrap some value in option or expr
#[macro_export]
macro_rules! unwrap_some_or {
    ($res: expr, $code: expr) => {
        match $res {
            Some(v) => v,
            None => $code,
        }
    };
}
