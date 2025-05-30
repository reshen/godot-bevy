# Timing Test Example

This example demonstrates the timing behavior of godot-bevy integration, showing how Bevy schedules run within Godot's frame callbacks.

> 📖 **For detailed information about timing and schedules**, see [docs/TIMING_AND_SCHEDULES.md](../../docs/TIMING_AND_SCHEDULES.md)

## What This Example Tests

This example helps you understand:

- **When different Bevy schedules execute** (First, PreUpdate, Update, FixedUpdate, PostUpdate, Last, PhysicsUpdate)
- **How often each schedule runs** relative to Godot's frame rate
- **The relationship between visual frames and physics frames**
- **How Bevy's FixedUpdate maintains consistent timing**

## What You'll See

The example logs periodic messages showing:

```
🚀 Timing Test Started!
📺 First Schedule Run #120: Time: 2.00s (runs in app.update())
🔄 PreUpdate running at 3.00s (part of app.update())
📋 Update running at 4.00s (part of app.update())
🔧 FixedUpdate Run #128: Time: 2.03s (Bevy's internal 64Hz timing)
📤 PostUpdate running at 5.00s (part of app.update())
🏁 Last Schedule: Update runs: 722, Physics runs: 365, Fixed updates: 384, Time: 6.00s
⚡ PhysicsUpdate Run #60: Time: 1.00s (runs in physics_process())
```

## Key Observations

### Frame Rates
- **Visual frames**: Run at your display's refresh rate (60-144 FPS)
- **Physics frames**: Run at Godot's physics tick rate (usually 60 Hz)
- **FixedUpdate**: Maintains Bevy's internal timing (64 Hz by default)

### Schedule Usage Guidelines

```rust
// For general game logic, UI, rendering - runs in visual frames
app.add_systems(Update, gameplay_system);

// For gameplay logic, AI - Bevy's built-in fixed timestep
app.add_systems(FixedUpdate, physics_simulation);

// For Godot-specific physics - synchronized with Godot's physics
app.add_systems(PhysicsUpdate, godot_movement_system);
```

## Running This Example

1. **Build**: `cargo build`
2. **Run**: You can either:
    1. Open the Godot project and run the scene
    1. Run: `cargo run`. NOTE: This requires the Godot binary, which we attempt
       to locate either through your environment's path or by searching common
       locations. If this doesn't work, update your path to include Godot. If
       this fails for other reasons, it may be because your version of Godot
       is different than the one the example was built with, in that case,
       try opening the Godot project first.
3. **Observe**: Watch the console output for timing patterns

## Understanding the Output

- **High visual frame rates** (100+ FPS) are normal and indicate good performance
- **FixedUpdate** may run 0, 1, or 2+ times per visual frame to maintain consistent timing
- **PhysicsUpdate** runs independently at Godot's physics rate
- **Timing consistency** shows that each schedule runs when expected

This example is particularly useful for:
- Understanding when to use each Bevy schedule
- Debugging timing-related issues
- Verifying frame rate expectations
- Learning about fixed timestep vs variable timestep systems

### Scheduling Relationships

**No Strong Ordering Guarantees**: `PhysicsUpdate` and the visual frame schedules (`First`, `PreUpdate`, `Update`, etc.) run independently. A physics frame might execute:
- Before a visual frame starts
- Between `PreUpdate` and `Update`
- After `Last` completes
- Multiple times between visual frames (if physics rate > visual rate)

**Data Synchronization**: Changes made in one schedule are visible in others, but with frame delays:
- Transform changes in `PhysicsUpdate` → visible in next visual frame's `Update`
- Transform changes in `Update` → visible in same or next `PhysicsUpdate`
- The transform sync systems (`PreUpdate`/`Last`) handle the bidirectional synchronization
