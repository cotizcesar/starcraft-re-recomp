//! Retail Remastered `.ui.json` dialog layouts.
//!
//! StarCraft Remastered keeps the classic 640x480 (and 320x240 sub-dialog)
//! control rectangles in `locales/enus/assets/rez/*.ui.json`.  The console,
//! score screen and main menu positions in this module are read from those
//! shipped files rather than hard-coded so the client uses the same
//! coordinates as the game.
//!
//! Only the subset of the UI schema the client needs is modelled: a dialog
//! rectangle and a flat list of child controls (type, id, content, rect,
//! flags).  Nested children are flattened with parent-relative rectangles
//! resolved against their parent, which is what the shipped dialogs use.

pub const UI_JSON: &[&str] = &[
    "minimap",
    "objctdlg",
    "tips_dlg",
    "statdata",
    "statres",
    "statport",
    "stat_f10",
    "statbtndefault",
    "statbtnn",
    "statbtnp",
    "statbtnt",
    "statbtnz",
    "statlb",
    "glumain",
    "gluscore",
    "gamemenu",
    "msnstats",
    "msnstat2",
    "msnstat4",
    "msnstat6",
    // Remastered front-end screens (episode/mission select, game type,
    // saved games, create/lobby, options, hotkeys, collections, credits).
    "glucmpgn",
    "gluexpcmpgn",
    "glugamemode",
    "gluload",
    "glucreat",
    "glugamelobby",
    "gluoptionspopup",
    "gluhotkeys",
    "glucollectionspopup",
    "credits",
    "creditsscreen",
    "lmission",
    "wmission",
    "titledlg",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl UiRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }

    pub fn offset(self, dx: i32, dy: i32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// Scale and translate into screen pixels.
    pub fn scaled(self, scale: f32, ox: i32, oy: i32) -> UiRect {
        UiRect::new(
            ox + (self.x as f32 * scale).round() as i32,
            oy + (self.y as f32 * scale).round() as i32,
            (self.w as f32 * scale).round() as i32,
            (self.h as f32 * scale).round() as i32,
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct UiControl {
    pub kind: String,
    pub id: i32,
    pub content: String,
    pub rect: UiRect,
    pub flags: Vec<String>,
    /// Absolute rectangle after parent offsets and flattened children.
    pub abs: UiRect,
    pub parent: Option<usize>,
}

impl UiControl {
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }
}

#[derive(Clone, Debug, Default)]
pub struct UiDialog {
    pub name: String,
    pub rect: UiRect,
    pub controls: Vec<UiControl>,
}

impl UiDialog {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let text =
            std::str::from_utf8(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes)).ok()?;
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let name = value.get("name")?.as_str()?.to_string();
        let rect = parse_rect(value.get("rect")?)?;
        let mut controls = Vec::new();
        let root = UiControl {
            kind: "DIALOG".into(),
            id: 0,
            content: name.clone(),
            rect,
            flags: Vec::new(),
            abs: rect,
            parent: None,
        };
        let base = controls.len();
        controls.push(root);
        if let Some(children) = value.get("children").and_then(|c| c.as_array()) {
            flatten(children, base, rect, &mut controls);
        }
        Some(Self {
            name,
            rect,
            controls,
        })
    }

    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        Self::parse(&bytes)
    }

    pub fn control(&self, id: i32) -> Option<&UiControl> {
        self.controls.iter().find(|c| c.id == id && c.id != 0)
    }

    pub fn controls_of<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a UiControl> + 'a {
        self.controls.iter().filter(move |c| c.kind == kind)
    }

    pub fn child(&self, id: i32, index: usize) -> Option<&UiControl> {
        self.controls.iter().filter(|c| c.id == id).nth(index)
    }

    pub fn by_content<'a>(&'a self, text: &'a str) -> impl Iterator<Item = &'a UiControl> + 'a {
        self.controls.iter().filter(move |c| c.content == text)
    }
}

fn parse_rect(value: &serde_json::Value) -> Option<UiRect> {
    let a = value.as_array()?;
    if a.len() < 4 {
        return None;
    }
    Some(UiRect::new(
        a[0].as_i64()? as i32,
        a[1].as_i64()? as i32,
        a[2].as_i64()? as i32,
        a[3].as_i64()? as i32,
    ))
}

fn flatten(
    children: &[serde_json::Value],
    parent_index: usize,
    parent_abs: UiRect,
    out: &mut Vec<UiControl>,
) {
    for child in children {
        let rect = child.get("rect").and_then(parse_rect).unwrap_or_default();
        let abs = UiRect::new(parent_abs.x + rect.x, parent_abs.y + rect.y, rect.w, rect.h);
        let index = out.len();
        out.push(UiControl {
            kind: child
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            id: child.get("id").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            content: child
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            rect,
            flags: child
                .get("flags")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|f| f.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            abs,
            parent: Some(parent_index),
        });
        if let Some(grand) = child.get("children").and_then(|c| c.as_array()) {
            flatten(grand, index, abs, out);
        }
    }
}

/// Load every `*.ui.json` in `dir` into a name-keyed map.
pub fn load_all(dir: &std::path::Path) -> std::collections::HashMap<String, UiDialog> {
    let mut out = std::collections::HashMap::new();
    for name in UI_JSON {
        let path = dir.join(format!("{name}.ui.json"));
        if let Some(dialog) = UiDialog::from_path(&path) {
            out.insert((*name).to_string(), dialog);
        }
    }
    out
}

/// Convenience: the screen rectangle of a control in a 640x480 layout,
/// scaled to the client viewport and centred in the classic 4:3 safe area.
pub fn screen_rect(
    dialog: &UiDialog,
    id: i32,
    scale: f32,
    safe_x: i32,
    safe_y: i32,
) -> Option<UiRect> {
    dialog
        .control(id)
        .map(|c| c.abs.scaled(scale, safe_x, safe_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> UiDialog {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/ui")
            .join(format!("{name}.ui.json"));
        UiDialog::from_path(&path).unwrap_or_else(|| panic!("{name} parses"))
    }

    #[test]
    fn every_materialised_layout_parses() {
        for name in UI_JSON {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/ui")
                .join(format!("{name}.ui.json"));
            let dialog =
                UiDialog::from_path(&path).unwrap_or_else(|| panic!("{name} does not parse"));
            assert!(!dialog.controls.is_empty(), "{name} has no controls");
        }
    }

    #[test]
    fn stat_dialogs_keep_the_shipped_exact_coordinates() {
        let data = fixture("statdata");
        assert_eq!(data.name, "StatData");
        assert_eq!(data.rect, UiRect::new(138, 388, 270, 92));
        // The nine command slots are 36x34 with the retail 46px column pitch.
        let btn = fixture("statbtndefault");
        assert_eq!(btn.rect, UiRect::new(496, 354, 144, 126));
        for row in 0..3 {
            for col in 0..3 {
                let id = row * 3 + col + 1;
                let c = btn.control(id).expect("slot");
                assert_eq!(c.rect.w, 36);
                assert_eq!(c.rect.h, 34);
                assert_eq!(c.abs.x, 496 + 9 + col * 46);
                assert_eq!(c.abs.y, 354 + 4 + row * 40);
            }
        }
        let port = fixture("statport");
        assert_eq!(port.rect, UiRect::new(408, 408, 88, 72));
        assert_eq!(port.control(1).unwrap().abs, UiRect::new(413, 410, 60, 56));
        let res = fixture("statres");
        assert_eq!(res.rect, UiRect::new(220, 0, 420, 20));
        let f10 = fixture("stat_f10");
        assert_eq!(f10.rect, UiRect::new(408, 388, 88, 20));
    }

    #[test]
    fn front_end_dialogs_keep_their_shipped_rects() {
        let episode = fixture("glucmpgn");
        assert_eq!(episode.rect, UiRect::new(0, 0, 640, 480));
        assert_eq!(
            episode.control(5).unwrap().abs,
            UiRect::new(243, 368, 189, 20),
            "Load Saved button"
        );
        assert_eq!(
            episode.control(9).unwrap().abs,
            UiRect::new(470, 404, 158, 20),
            "Cancel button"
        );
        let load = fixture("gluload");
        assert_eq!(load.control(4).unwrap().abs, UiRect::new(441, 383, 152, 22));
        let game_type = fixture("glugamemode");
        assert_eq!(game_type.rect, UiRect::new(0, 0, 360, 200));
        let credits = fixture("creditsscreen");
        assert_eq!(
            credits.control(2).unwrap().abs,
            UiRect::new(120, 30, 420, 450)
        );
        let hotkeys = fixture("gluhotkeys");
        assert_eq!(hotkeys.rect, UiRect::new(0, 0, 640, 480));
    }

    #[test]
    fn main_menu_and_score_layouts_match_the_640x480_dialogs() {
        let menu = fixture("glumain");
        assert_eq!(menu.rect, UiRect::new(0, 0, 640, 480));
        let exit = menu.control(2).expect("exit button");
        assert_eq!(exit.rect, UiRect::new(395, 320, 231, 136));
        // The single-player item is a full-width hot area at the top left.
        let single = menu.control(21).expect("single player");
        assert_eq!(single.rect, UiRect::new(0, 0, 320, 190));
        assert_eq!(
            menu.control(22).unwrap().abs,
            UiRect::new(413, 66, 205, 205)
        );
        assert_eq!(
            menu.control(23).unwrap().abs,
            UiRect::new(374, 14, 266, 264)
        );

        let score = fixture("gluscore");
        assert_eq!(score.rect, UiRect::new(0, 0, 640, 480));
        assert_eq!(
            score.control(7).unwrap().abs,
            UiRect::new(480, 430, 138, 30)
        );
        // Eight translucent score rows, each 588x37.
        let rows: Vec<_> = score
            .controls_of("RECT")
            .filter(|c| c.rect.w == 588)
            .collect();
        assert_eq!(rows.len(), 8);
        assert_eq!(rows[0].abs.y, 76);
        assert_eq!(rows[7].abs.y, 356);

        let mission = fixture("msnstats");
        assert_eq!(mission.rect, UiRect::new(0, 0, 320, 240));
        assert_eq!(mission.control(1).unwrap().abs, UiRect::new(6, 12, 96, 25));

        let pause = fixture("gamemenu");
        assert_eq!(pause.rect, UiRect::new(184, 32, 264, 288));
        assert_eq!(pause.control(1).unwrap().abs, UiRect::new(204, 68, 224, 28));
        assert_eq!(
            pause.control(-3).unwrap().abs,
            UiRect::new(204, 284, 224, 28)
        );

        let lb = fixture("statlb");
        assert_eq!(lb.rect, UiRect::new(0, 0, 200, 112));
        assert_eq!(lb.control(-10).unwrap().abs, UiRect::new(2, 2, 10, 10));
        assert_eq!(lb.control(-47).unwrap().abs, UiRect::new(100, 98, 60, 14));
    }
}
