#[path = "../mixins/draimhorizon/block_protection.rs"]
pub mod block_protection;
#[path = "../mixins/draimhorizon/runtime.rs"]
pub mod claims;
#[path = "../mixins/draimhorizon/overlay.rs"]
pub mod overlay;
#[path = "../mixins/draimhorizon/party.rs"]
pub mod party;
#[path = "../mixins/draimhorizon/pat_arms.rs"]
pub mod pat_arms;
#[path = "../mixins/draimhorizon/visibility.rs"]
pub mod visibility;
#[path = "../mixins/draimhorizon/waypoints.rs"]
pub mod waypoints;
pub use waypoints as claim_borders_waypoints;

#[path = "../mixins/draimhorizon/visualization.rs"]
pub mod visualization;

#[path = "../mixins/draimhorizon/menu.rs"]
pub mod menu;
#[path = "../mixins/draimhorizon/navigation.rs"]
pub mod navigation;
pub use navigation as claim_borders_navigation;
#[path = "../mixins/draimhorizon/mining.rs"]
pub mod mining;
pub use mining as claim_borders_mining;
#[path = "../mixins/draimhorizon/construction.rs"]
pub mod construction;
pub use construction as claim_borders_construction;
#[path = "../mixins/draimhorizon/schematic.rs"]
pub mod schematic;
pub use schematic as claim_borders_schematic;
#[path = "../mixins/draimhorizon/farming.rs"]
pub mod farming;
pub use farming as claim_borders_farming;
#[path = "../mixins/draimhorizon/farm_plots.rs"]
pub mod farm_plots;
pub use farm_plots as claim_borders_farm_plots;
#[path = "../mixins/draimhorizon/draining.rs"]
pub mod draining;
pub use draining as claim_borders_draining;
#[path = "../mixins/draimhorizon/autoclick.rs"]
pub mod autoclick;
pub use autoclick as claim_borders_autoclick;
#[path = "../mixins/draimhorizon/schematic_share.rs"]
pub mod schematic_share;
pub use schematic_share as claim_borders_schematic_share;
#[path = "../mixins/draimhorizon/schematic_places.rs"]
pub mod schematic_places;
pub use schematic_places as claim_borders_schematic_places;
#[path = "../mixins/draimhorizon/shulker_peek.rs"]
pub mod shulker_peek;
pub use shulker_peek as claim_borders_shulker_peek;
#[path = "../mixins/draimhorizon/auto_eat.rs"]
pub mod auto_eat;
pub use auto_eat as claim_borders_auto_eat;
#[path = "../mixins/draimhorizon/farm_storage.rs"]
pub mod farm_storage;
pub use farm_storage as claim_borders_farm_storage;

#[path = "../mixins/draimhorizon/relay_settings.rs"]
pub mod relay_settings;

#[path = "../mixins/draimhorizon/chat_feed.rs"]
pub mod chat_feed;

#[path = "../mixins/draimhorizon/chat_preferences.rs"]
pub mod chat_preferences;
pub use chat_preferences as claim_borders_chat_preferences;

#[path = "../mixins/draimhorizon/claim_map.rs"]
pub mod claim_map;
pub use claim_map as claim_borders_map;

#[path = "../mixins/draimhorizon/inventory_sort.rs"]
pub mod inventory_sort;

#[path = "../mixins/draimhorizon/map_history.rs"]
pub mod map_history;
pub use map_history as claim_borders_map_history;
#[path = "../mixins/draimhorizon/map_worker.rs"]
pub mod map_worker;

pub use claims as claim_borders;
#[path = "../mixins/draimhorizon/claim_directory.rs"]
pub mod claim_directory;
#[path = "../mixins/draimhorizon/outgoing.rs"]
pub mod outgoing;
#[path = "../mixins/draimhorizon/teleport.rs"]
pub mod teleport;

#[path = "../mixins/draimhorizon/auto_trust.rs"]
pub mod auto_trust;
#[path = "../mixins/draimhorizon/economy.rs"]
pub mod economy;
#[path = "../mixins/draimhorizon/economy_persistence.rs"]
pub mod economy_persistence;
#[path = "../mixins/draimhorizon/economy_scan.rs"]
pub mod economy_scan;

#[path = "../mixins/draimhorizon/chat_priority.rs"]
pub mod chat_priority;
pub use chat_priority as claim_borders_chat_priority;

pub use economy as claim_borders_economy;
#[path = "../mixins/draimhorizon/economy_requests.rs"]
pub mod economy_requests;
#[path = "../mixins/draimhorizon/economy_ui.rs"]
pub mod economy_ui;

#[path = "../mixins/draimhorizon/claim_observations.rs"]
pub mod claim_observations;

#[path = "../mixins/draimhorizon/server_tps.rs"]
pub mod server_tps;

pub use inventory_sort as claim_borders_inventory_sort;
#[path = "../mixins/draimhorizon/clickthrough.rs"]
pub mod clickthrough;
#[path = "../mixins/draimhorizon/inventory_preferences.rs"]
pub mod inventory_preferences;

#[path = "../mixins/draimhorizon/card_layout.rs"]
pub mod card_layout;
pub use card_layout as claim_borders_card_layout;
#[path = "../mixins/draimhorizon/shape_cache.rs"]
pub mod shape_cache;
#[path = "../mixins/draimhorizon/text_layout.rs"]
pub mod text_layout;
pub use shape_cache as claim_borders_shape_cache;
pub use text_layout as claim_borders_text_layout;
#[path = "../mixins/draimhorizon/translation_plan.rs"]
pub mod translation_plan;
#[path = "../mixins/draimhorizon/ui_style.rs"]
pub mod ui_style;
pub use ui_style as claim_borders_ui_style;

#[path = "../mixins/draimhorizon/input.rs"]
pub mod input;

#[path = "../mixins/draimhorizon/seed_map.rs"]
pub mod seed_map;
pub use seed_map as claim_borders_seed_map;
#[path = "../mixins/draimhorizon/seed_poi.rs"]
pub mod seed_poi;

#[path = "../mixins/draimhorizon/experimental.rs"]
pub mod experimental;
pub use experimental as claim_borders_experimental;

pub use chat_feed as claim_borders_chat_feed;
#[path = "../mixins/draimhorizon/chat_archive.rs"]
pub mod chat_archive;
pub use chat_archive as claim_borders_chat_archive;

#[path = "../mixins/draimhorizon/reconnect.rs"]
pub mod reconnect;

#[path = "../mixins/draimhorizon/patting.rs"]
pub mod patting;
pub use patting as claim_borders_patting;

#[path = "../mixins/draimhorizon/baritone_walk.rs"]
pub mod baritone_walk;
pub use baritone_walk as claim_borders_baritone_walk;

#[path = "../mixins/draimhorizon/baritone_profile.rs"]
pub mod baritone_profile;
pub use baritone_profile as claim_borders_baritone_profile;

#[path = "../mixins/draimhorizon/mine_picker.rs"]
pub mod mine_picker;
pub use mine_picker as claim_borders_mine_picker;

#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/schematic/mask.rs"]
pub mod baritone_schematic_mask;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/schematic/schematic.rs"]
pub mod baritone_schematic;
#[path = "../mixins/draimhorizon/baritone_shapes.rs"]
pub mod baritone_shapes;
pub use baritone_shapes as claim_borders_baritone_shapes;

// Baritone navigation (LGPL-3.0, see mixins/draimhorizon/baritone/NOTICE.md).
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/api/utils.rs"]
pub mod baritone_utils;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/costs.rs"]
pub mod baritone_costs;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/api/goals.rs"]
pub mod baritone_goals;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/api/block_optional_meta.rs"]
pub mod baritone_bom;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/cache/world_scanner.rs"]
pub mod baritone_world_scanner;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/cache/cached_world.rs"]
pub mod baritone_cached_world;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/api/settings.rs"]
pub mod baritone_settings;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/cache/waypoints.rs"]
pub mod baritone_waypoints;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/utils/block_state.rs"]
pub mod baritone_block_state;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/utils/block_state_interface.rs"]
pub mod baritone_block_state_interface;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/calculation_context.rs"]
pub mod baritone_calculation_context;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/movement_helper.rs"]
pub mod baritone_movement_helper;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/movements/traverse.rs"]
pub mod baritone_movement_traverse;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/movements/ascend.rs"]
pub mod baritone_movement_ascend;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/movements/descend.rs"]
pub mod baritone_movement_descend;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/movements/diagonal.rs"]
pub mod baritone_movement_diagonal;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/movements/downward.rs"]
pub mod baritone_movement_downward;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/movements/parkour.rs"]
pub mod baritone_movement_parkour;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/movements/pillar.rs"]
pub mod baritone_movement_pillar;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/moves.rs"]
pub mod baritone_moves;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/movement.rs"]
pub mod baritone_movement;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/calc/a_star.rs"]
pub mod baritone_astar;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/calc/path.rs"]
pub mod baritone_calc_path;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/utils/rotation.rs"]
pub mod baritone_rotation;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/behavior/aim.rs"]
pub mod baritone_aim;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/behavior/look.rs"]
pub mod baritone_look;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/behavior/movement_state.rs"]
pub mod baritone_movement_state;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/behavior/player_context.rs"]
pub mod baritone_player_context;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/movement/movement_exec.rs"]
pub mod baritone_movement_exec;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/avoidance.rs"]
pub mod baritone_avoidance;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/path/sprint_jump.rs"]
pub mod baritone_sprint_jump;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/pathing/path/executor.rs"]
pub mod baritone_executor;
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/behavior/pathing_behavior.rs"]
pub mod baritone_pathing_behavior;
#[cfg(test)]
#[rustfmt::skip]
#[allow(clippy::all, dead_code, unused_mut)]
#[path = "../mixins/draimhorizon/baritone/tests_pathing.rs"]
mod baritone_tests_pathing;
