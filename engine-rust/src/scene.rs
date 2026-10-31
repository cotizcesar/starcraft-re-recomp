//! Reference scenes: the unit layout the retail screenshots show.
//!
//! The in-game captures in `docs/original_screenshots` are not empty terrain —
//! reference #30, for example, is a late-game Terran base (command centre,
//! supply-depot grid, factory, refinery, goliaths, vultures). The shipped
//! replays carry no action stream, so the state cannot be replayed; instead it
//! is reconstructed here from the screenshot itself.
//!
//! Coordinates are **screen pixels inside the 16:9 content box** (the box the
//! retail client pillarboxes into 440..2999 at 3440x1440). The retail client
//! draws the SD world at two screen pixels per world pixel, so a scene unit's
//! world position is `camera + (screen - content_left) / SCREEN_PER_WORLD`.
//!
//! Every entry was read off the reference: the supply-depot grid, for example,
//! sits on the green glow of each depot, which measures x 710/998/1288 and
//! y 620/812/1004 in reference #30.

use crate::ids::unit;

/// Screen pixels per world pixel of the project's world viewport (calibrated
/// by measurement: the divisor that matches the retail references best, and the
/// one the scene coordinates in this file were read at).
pub const SCREEN_PER_WORLD: i32 = 3;

/// One unit of a reconstructed scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneUnit {
    pub type_id: u16,
    /// Centre of the sprite, in content-box screen pixels.
    pub screen: (i32, i32),
}

/// A reconstructed reference frame.
#[derive(Clone, Copy, Debug)]
pub struct ReferenceScene {
    /// Camera anchor in world pixels (the scene is placed relative to it).
    pub camera: (i32, i32),
    pub units: &'static [SceneUnit],
}

const fn u(type_id: u16, x: i32, y: i32) -> SceneUnit {
    SceneUnit {
        type_id,
        screen: (x, y),
    }
}

/// Reference #30-#34: a Terran base on badlands. Buildings whose pixel centre
/// sits off-frame are left out rather than guessed.
pub const TERRAN_BASE: &[SceneUnit] = &[
    // Supply-depot grid: three columns at x 710/998/1288, three rows whose
    // green glow measures y 620/812/1004 in the reference. Our art carries the
    // glow 58 px higher inside the sprite, so the sprite centre is 58 px below
    // the glow Y read off the screenshot (measured by correlating the two).
    u(unit::SUPPLY_DEPOT, 710, 648),
    u(unit::SUPPLY_DEPOT, 998, 648),
    u(unit::SUPPLY_DEPOT, 1288, 648),
    u(unit::SUPPLY_DEPOT, 710, 840),
    u(unit::SUPPLY_DEPOT, 998, 840),
    u(unit::SUPPLY_DEPOT, 1288, 840),
    u(unit::SUPPLY_DEPOT, 710, 1032),
    u(unit::SUPPLY_DEPOT, 998, 1032),
    u(unit::SUPPLY_DEPOT, 1288, 1032),
    // Top row of the base: barracks, engineering bay, factory, refinery,
    // another depot, the command centre and a second barracks.
    u(unit::BARRACKS, 600, 60),
    u(unit::ENGINEERING_BAY, 1000, 155),
    u(unit::FACTORY, 1330, 165),
    u(unit::REFINERY, 1600, 80),
    u(unit::SUPPLY_DEPOT, 1880, 75),
    u(unit::COMMAND_CENTER, 2850, 300),
    u(unit::BARRACKS, 2015, 380),
    // Goliath group in the middle-right.
    u(unit::GOLIATH, 2180, 150),
    u(unit::GOLIATH, 2260, 230),
    u(unit::GOLIATH, 2340, 150),
    u(unit::GOLIATH, 2420, 290),
    u(unit::GOLIATH, 2520, 210),
    // Vultures guarding the left approach, marines by the barracks.
    u(unit::VULTURE, 620, 1250),
    u(unit::VULTURE, 700, 1330),
    u(unit::VULTURE, 780, 1250),
    u(unit::MARINE, 2180, 300),
    u(unit::MARINE, 2240, 340),
    u(unit::MARINE, 2300, 380),
    u(unit::SIEGE_TANK, 1700, 320),
    u(unit::SIEGE_TANK, 1780, 400),
];

const TERRAN_SCENE: ReferenceScene = ReferenceScene {
    camera: (1024, 1024),
    units: TERRAN_BASE,
};

/// The scene for a capture screen, if one has been reconstructed.
pub fn reference_scene(name: &str) -> Option<ReferenceScene> {
    match name {
        "ingame-terran" | "ingame-terran2" | "ingame" => Some(TERRAN_SCENE),
        _ => None,
    }
}

/// World position of a scene unit, given the camera, the content-box left and
/// how many screen pixels the client draws per world pixel.
pub fn world_of(
    camera: (i32, i32),
    content_left: i32,
    screen: (i32, i32),
    screen_per_world: i32,
) -> (i32, i32) {
    let k = screen_per_world.max(1);
    (
        camera.0 + (screen.0 - content_left) / k,
        camera.1 + screen.1 / k,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scene_unit_sits_inside_the_content_box() {
        for unit in TERRAN_BASE {
            assert!(
                (440..3000).contains(&unit.screen.0),
                "x {} off the content box",
                unit.screen.0
            );
            assert!(
                (0..1440).contains(&unit.screen.1),
                "y {} off the content box",
                unit.screen.1
            );
        }
    }

    #[test]
    fn the_terran_base_has_its_main_structures() {
        let count = |id: u16| TERRAN_BASE.iter().filter(|s| s.type_id == id).count();
        assert_eq!(count(unit::SUPPLY_DEPOT), 10, "supply-depot grid + one");
        assert_eq!(count(unit::COMMAND_CENTER), 1);
        assert_eq!(count(unit::FACTORY), 1);
        assert_eq!(count(unit::REFINERY), 1);
        assert!(count(unit::GOLIATH) >= 4);
    }

    #[test]
    fn screen_pixels_map_to_world_pixels_at_the_retail_scale() {
        // Content left at 3440x1440 pillarboxing is 440, and the retail client
        // draws two screen pixels per world pixel.
        assert_eq!(world_of((100, 200), 440, (440, 0), 2), (100, 200));
        assert_eq!(world_of((100, 200), 440, (640, 100), 2), (200, 250));
        // Moving the camera moves the whole scene by the same amount.
        assert_eq!(world_of((150, 260), 440, (640, 100), 2), (250, 310));
        // At 3x the same screen spot is closer to the camera.
        assert_eq!(world_of((100, 200), 440, (740, 300), 3), (200, 300));
    }

    #[test]
    fn captures_without_a_scene_fall_back_to_the_map() {
        assert!(reference_scene("ingame-zerg").is_none());
        assert!(reference_scene("ingame-terran").is_some());
    }
}
