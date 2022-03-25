#![allow(unused_macros)]

macro_rules! cfg_wasm32 {
    ($($item:item)*) => {
        $(
            #[cfg(target_arch = "wasm32")]
            $item
        )*
    }
}

macro_rules! cfg_not_wasm32 {
    ($($item:item)*) => {
        $(
            #[cfg(not(target_arch = "wasm32"))]
            $item
        )*
    }
}

macro_rules! cfg_run_example {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "run-example")]
            #[cfg_attr(docsrs, doc(cfg(feature = "run-example")))]
            $item
        )*
    }
}

macro_rules! cfg_wasm_opt {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "wasm-opt")]
            #[cfg_attr(docsrs, doc(cfg(feature = "wasm-opt")))]
            $item
        )*
    }
}

macro_rules! cfg_sass {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "sass")]
            #[cfg_attr(docsrs, doc(cfg(feature = "sass")))]
            $item
        )*
    }
}
