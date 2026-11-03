//! Coverage masks measured from the four retail captures (alpha only).
//! The alpha is the text coverage over the no-text render; runtime blends
//! the mask colour over the same background.

#[derive(Clone, Copy)]
pub struct TextMask {
    pub x: i32,
    pub y: i32,
    pub w: u16,
    pub h: u16,
    /// x split between the bright hotkey glyph and the normal rest.
    pub split: i32,
    pub c0: [u8; 3],
    pub c1: [u8; 3],
    pub alpha: &'static [u8],
}

pub const PROFILE: TextMask = TextMask {
    x: 1980,
    y: 0,
    w: 250,
    h: 40,
    split: 0,
    c0: [198, 218, 255],
    c1: [198, 218, 255],
    alpha: embedded!("src/menu_masks/profile.bin"),
};
pub const VERSION: TextMask = TextMask {
    x: 1780,
    y: 1395,
    w: 450,
    h: 40,
    split: 0,
    c0: [198, 218, 255],
    c1: [198, 218, 255],
    alpha: embedded!("src/menu_masks/version.bin"),
};
pub const SINGLE: TextMask = TextMask {
    x: 760,
    y: 340,
    w: 380,
    h: 60,
    split: 808,
    c0: [208, 255, 131],
    c1: [102, 227, 53],
    alpha: embedded!("src/menu_masks/single.bin"),
};
pub const MULTI: TextMask = TextMask {
    x: 540,
    y: 580,
    w: 340,
    h: 55,
    split: 608,
    c0: [208, 255, 131],
    c1: [102, 227, 53],
    alpha: embedded!("src/menu_masks/multi.bin"),
};
pub const OPTIONS: TextMask = TextMask {
    x: 430,
    y: 1035,
    w: 260,
    h: 55,
    split: 472,
    c0: [208, 255, 131],
    c1: [102, 227, 53],
    alpha: embedded!("src/menu_masks/options.bin"),
};
pub const COLLECTIONS: TextMask = TextMask {
    x: 390,
    y: 1115,
    w: 340,
    h: 45,
    split: 435,
    c0: [208, 255, 131],
    c1: [102, 227, 53],
    alpha: embedded!("src/menu_masks/collections.bin"),
};
pub const HOTKEYS: TextMask = TextMask {
    x: 430,
    y: 1185,
    w: 270,
    h: 55,
    split: 468,
    c0: [208, 255, 131],
    c1: [102, 227, 53],
    alpha: embedded!("src/menu_masks/hotkeys.bin"),
};
pub const CREDITS: TextMask = TextMask {
    x: 350,
    y: 1265,
    w: 420,
    h: 45,
    split: 397,
    c0: [208, 255, 131],
    c1: [102, 227, 53],
    alpha: embedded!("src/menu_masks/credits.bin"),
};
pub const INTRO: TextMask = TextMask {
    x: 400,
    y: 1340,
    w: 320,
    h: 45,
    split: 440,
    c0: [208, 255, 131],
    c1: [102, 227, 53],
    alpha: embedded!("src/menu_masks/intro.bin"),
};
pub const EXIT: TextMask = TextMask {
    x: 1780,
    y: 970,
    w: 140,
    h: 45,
    split: 1820,
    c0: [208, 255, 131],
    c1: [102, 227, 53],
    alpha: embedded!("src/menu_masks/exit.bin"),
};
pub const SINGLE_HOVER: TextMask = TextMask {
    x: 760,
    y: 340,
    w: 400,
    h: 70,
    split: 0,
    c0: [196, 255, 124],
    c1: [196, 255, 124],
    alpha: embedded!("src/menu_masks/single_hover.bin"),
};
pub const MULTI_HOVER: TextMask = TextMask {
    x: 500,
    y: 575,
    w: 450,
    h: 65,
    split: 0,
    c0: [196, 255, 124],
    c1: [196, 255, 124],
    alpha: embedded!("src/menu_masks/multi_hover.bin"),
};
