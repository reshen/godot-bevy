#![allow(deprecated)] // TODO: remove this once we've removed SystemDeltaTimer

use bevy::app::{App, Plugin, ScheduleRunnerPlugin};
use bevy::asset::{
    AssetMetaCheck, AssetPlugin,
    io::{AssetSource, AssetSourceId},
};
use bevy::ecs::schedule::{Schedule, ScheduleLabel};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::any::TypeId;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

/// Schedule that runs during Godot's physics_process at physics frame rate.
/// Use this for movement, physics, and systems that need to sync with Godot's physics timing.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PhysicsUpdate;

/// Resource containing Godot's physics delta time for the current frame
#[derive(Resource, Default)]
pub struct PhysicsDelta {
    pub delta_seconds: f32,
}

impl PhysicsDelta {
    pub fn new(delta: f64) -> Self {
        Self {
            delta_seconds: delta as f32,
        }
    }

    pub fn delta(&self) -> Duration {
        Duration::from_secs_f32(self.delta_seconds)
    }
}

/// Resource marker to ensure systems accessing Godot APIs run on the main thread
#[derive(Resource, Default)]
pub struct MainThreadMarker;

/// Transform synchronization modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformSyncMode {
    /// No transform syncing - use direct Godot physics (move_and_slide, etc.)
    /// Best for: Platformers, physics-heavy games
    Disabled,
    /// One-way sync: ECS → Godot only
    /// Best for: Pure ECS games, simple movement
    OneWay,
    /// Two-way sync: ECS ↔ Godot
    /// Best for: Hybrid apps migrating from GDScript to ECS
    TwoWay,
}

impl Default for TransformSyncMode {
    fn default() -> Self {
        Self::OneWay
    }
}

/// Configuration resource for transform syncing behavior
#[derive(Resource, Debug, Clone)]
pub struct GodotTransformConfig {
    pub sync_mode: TransformSyncMode,
}

impl Default for GodotTransformConfig {
    fn default() -> Self {
        Self {
            sync_mode: TransformSyncMode::OneWay,
        }
    }
}

impl GodotTransformConfig {
    /// Disable all transform syncing - use direct Godot physics instead
    pub fn disabled() -> Self {
        Self {
            sync_mode: TransformSyncMode::Disabled,
        }
    }

    /// Enable one-way sync (ECS → Godot) - default behavior
    pub fn one_way() -> Self {
        Self {
            sync_mode: TransformSyncMode::OneWay,
        }
    }

    /// Enable two-way sync (ECS ↔ Godot) for hybrid apps
    pub fn two_way() -> Self {
        Self {
            sync_mode: TransformSyncMode::TwoWay,
        }
    }
}

use crate::interop::GodotNodeHandle;
use bevy::ecs::system::EntityCommands;

/// Function that adds a component to an entity with access to the Godot node
type ComponentInserter = Box<dyn Fn(&mut EntityCommands, &GodotNodeHandle) + Send + Sync>;

/// Registry for components that should be added to entities spawned from the scene tree
#[derive(Resource, Default)]
pub struct SceneTreeComponentRegistry {
    /// Components to add to every entity spawned from scene tree
    /// Stored as (TypeId, inserter) to avoid duplicates
    components: Vec<(TypeId, ComponentInserter)>,
}

impl SceneTreeComponentRegistry {
    /// Register a component type to be added to all scene tree entities
    pub fn register<C>(&mut self)
    where
        C: Component + Default,
    {
        let type_id = TypeId::of::<C>();

        // Check if already registered
        if self.components.iter().any(|(id, _)| *id == type_id) {
            return;
        }

        let inserter = Box::new(|entity: &mut EntityCommands, _node: &GodotNodeHandle| {
            entity.insert(C::default());
        });
        self.components.push((type_id, inserter));
    }

    /// Register a component type with custom initialization logic
    pub fn register_with_init<C, F>(&mut self, init_fn: F)
    where
        C: Component,
        F: Fn(&mut EntityCommands, &GodotNodeHandle) + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<C>();

        // Check if already registered
        if self.components.iter().any(|(id, _)| *id == type_id) {
            return;
        }

        let inserter = Box::new(init_fn);
        self.components.push((type_id, inserter));
    }

    /// Add all registered components to an entity
    pub fn add_to_entity(&self, entity: &mut EntityCommands, node: &GodotNodeHandle) {
        for (_, inserter) in &self.components {
            inserter(entity, node);
        }
    }
}

/// Extension trait for App to register scene tree components
pub trait AppSceneTreeExt {
    /// Register a component to be added to all scene tree entities with default value
    fn register_scene_tree_component<C>(&mut self) -> &mut Self
    where
        C: Component + Default;

    /// Register a component with custom initialization logic that has access to the Godot node
    fn register_scene_tree_component_with_init<C, F>(&mut self, init_fn: F) -> &mut Self
    where
        C: Component,
        F: Fn(&mut EntityCommands, &GodotNodeHandle) + Send + Sync + 'static;
}

impl AppSceneTreeExt for App {
    fn register_scene_tree_component<C>(&mut self) -> &mut Self
    where
        C: Component + Default,
    {
        // Get or create the registry
        if !self
            .world()
            .contains_resource::<SceneTreeComponentRegistry>()
        {
            self.world_mut()
                .init_resource::<SceneTreeComponentRegistry>();
        }

        self.world_mut()
            .resource_mut::<SceneTreeComponentRegistry>()
            .register::<C>();

        self
    }

    fn register_scene_tree_component_with_init<C, F>(&mut self, init_fn: F) -> &mut Self
    where
        C: Component,
        F: Fn(&mut EntityCommands, &GodotNodeHandle) + Send + Sync + 'static,
    {
        // Get or create the registry
        if !self
            .world()
            .contains_resource::<SceneTreeComponentRegistry>()
        {
            self.world_mut()
                .init_resource::<SceneTreeComponentRegistry>();
        }

        self.world_mut()
            .resource_mut::<SceneTreeComponentRegistry>()
            .register_with_init::<C, F>(init_fn);

        self
    }
}

/// Minimal core plugin with only essential Godot-Bevy integration.
/// This includes scene tree management, basic Bevy setup, and core resources.
#[derive(Default)]
pub struct GodotBaseCorePlugin;

impl Plugin for GodotBaseCorePlugin {
    fn build(&self, app: &mut App) {
        // IMPORTANT: Register custom AssetReader BEFORE setting up AssetPlugin
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSource::build()
                .with_reader(|| Box::new(crate::plugins::assets::GodotAssetReader::new())),
        );
        app.register_asset_source(
            AssetSourceId::from("res"),
            AssetSource::build()
                .with_reader(|| Box::new(crate::plugins::assets::GodotAssetReader::new())),
        );
        app.register_asset_source(
            AssetSourceId::from("user"),
            AssetSource::build()
                .with_reader(|| Box::new(crate::plugins::assets::GodotAssetReader::new())),
        );
        app.register_asset_source(
            AssetSourceId::from("uid"),
            AssetSource::build()
                .with_reader(|| Box::new(crate::plugins::assets::GodotAssetReader::new())),
        );

        app.add_plugins(MinimalPlugins.build().disable::<ScheduleRunnerPlugin>())
            // Configure AssetPlugin to bypass path verification for Godot resources
            .add_plugins(AssetPlugin {
                meta_check: AssetMetaCheck::Never,
                ..default()
            })
            .add_plugins(bevy::log::LogPlugin::default())
            .add_plugins(bevy::diagnostic::DiagnosticsPlugin)
            .init_resource::<PhysicsDelta>()
            .init_non_send_resource::<MainThreadMarker>()
            .init_resource::<SceneTreeComponentRegistry>();

        // Add the PhysicsUpdate schedule
        app.add_schedule(Schedule::new(PhysicsUpdate));
    }
}

/// SystemParam to keep track of an independent delta time
///
/// Not every system runs on a Bevy update and Bevy can be updated multiple
/// during a "frame".
#[derive(SystemParam)]
#[deprecated(note = "Use PhysicsDelta instead")]
pub struct SystemDeltaTimer<'w, 's> {
    last_time: Local<'s, Option<Instant>>,
    marker: PhantomData<&'w ()>,
}

#[allow(deprecated)]
impl<'w, 's> SystemDeltaTimer<'w, 's> {
    /// Returns the time passed since the last invocation
    pub fn delta(&mut self) -> Duration {
        let now = Instant::now();
        let last_time = self.last_time.unwrap_or(now);

        *self.last_time = Some(now);

        now - last_time
    }

    pub fn delta_seconds(&mut self) -> f32 {
        self.delta().as_secs_f32()
    }

    pub fn delta_seconds_f64(&mut self) -> f64 {
        self.delta().as_secs_f64()
    }
}

pub trait FindEntityByNameExt<T> {
    fn find_entity_by_name(self, name: &str) -> Option<T>;
}

impl<'a, T: 'a, U> FindEntityByNameExt<T> for U
where
    U: Iterator<Item = (&'a Name, T)>,
{
    fn find_entity_by_name(mut self, name: &str) -> Option<T> {
        self.find_map(|(ent_name, t)| (ent_name.as_str() == name).then_some(t))
    }
}
