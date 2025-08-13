pub mod vec2d;

/// macro for compile-time const assertions
macro_rules! const_assert {
    ($message:expr, $($list:ident : $ty:ty),* => $expr:expr) => {{
        struct Assert<$(const $list: $ty,)*>;
        impl<$(const $list: $ty,)*> Assert<$($list,)*> {
            const OK: () = {
                if !($expr) {
                    ::core::panic!(::core::concat!("assertion failed: ", $message));
                }
            };
        }
        Assert::<$($list,)*>::OK
    }};
}

pub(crate) use const_assert;
