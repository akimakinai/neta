use bevy::{
    ecs::{component::HookContext, system::IntoObserverSystem, world::DeferredWorld},
    prelude::*,
};
use core::marker::PhantomData;
use core::mem;

#[derive(Component)]
#[component(on_insert = on_insert_observe::<E, B>)]
#[component(on_replace = on_replace_observe::<E, B>)]
/// A component that spawns an observer entity when added.
/// Note that replacing or removing this component will despawn the old observer.
pub enum Observe<E: Event, B: Bundle = ()> {
    Added(Observer, PhantomData<fn(E, B)>),
    Observed(Entity),
}

impl<E: Event, B: Bundle> Observe<E, B> {
    pub fn new<M>(obs: impl IntoObserverSystem<E, B, M>) -> Self {
        Self::Added(Observer::new(obs), PhantomData)
    }
}

fn on_insert_observe<E: Event, B: Bundle>(mut world: DeferredWorld, context: HookContext) {
    let (mut entities, mut commands) = world.entities_and_commands();

    let Ok(mut observe) = entities.get_mut(context.entity) else {
        unreachable!();
    };
    let Some(mut observe) = observe.get_mut::<Observe<E, B>>() else {
        // on_insert hook is called after the component is inserted
        unreachable!();
    };

    let mut obs_entity = commands.spawn_empty();
    let obs_entity_id = obs_entity.id();

    let Observe::Added(obs, _) = mem::replace(&mut *observe, Observe::Observed(obs_entity_id))
    else {
        error!("Invalid `Observe` component is added");
        commands.entity(obs_entity_id).despawn();
        return;
    };

    obs_entity.insert(obs.with_entity(context.entity));
}

fn on_replace_observe<E: Event, B: Bundle>(mut world: DeferredWorld, context: HookContext) {
    let Some(observed) = world.get::<Observe<E, B>>(context.entity) else {
        unreachable!();
    };

    let Observe::Observed(obs_id) = *observed else {
        error!("Invalid `Observe` component is replaced or removed");
        return;
    };

    world.commands().queue(move |world: &mut World| {
        if world.get_entity(context.entity).is_err() {
            // All observers will be despawned by `ObservedBy` on_remove hook
            return;
        }

        if let Ok(entity) = world.get_entity_mut(obs_id) {
            entity.despawn();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observe_component_addition() {
        #[derive(Event)]
        struct TestEvent;

        #[derive(Resource, PartialEq, Debug)]
        struct CheckTriggered(u32);

        let mut world = World::new();

        world.insert_resource(CheckTriggered(0));

        assert_eq!(world.query::<&Observer>().iter(&world).count(), 0);

        let observer_system = |_trigger: Trigger<TestEvent>, mut check: ResMut<CheckTriggered>| {
            check.0 += 1;
        };

        let entity = world
            .spawn(Observe::<TestEvent, ()>::new(observer_system))
            .id();

        assert_eq!(
            *world.get_resource::<CheckTriggered>().unwrap(),
            CheckTriggered(0),
        );
        assert_eq!(world.query::<&Observer>().iter(&world).count(), 1);

        world.entity_mut(entity).trigger(TestEvent);

        assert_eq!(
            *world.get_resource::<CheckTriggered>().unwrap(),
            CheckTriggered(1),
        );

        let observer_system2 = |_trigger: Trigger<TestEvent>, mut check: ResMut<CheckTriggered>| {
            check.0 += 2;
        };

        world
            .entity_mut(entity)
            .insert(Observe::<TestEvent, ()>::new(observer_system2))
            .trigger(TestEvent);

        assert_eq!(
            *world.get_resource::<CheckTriggered>().unwrap(),
            CheckTriggered(3),
        );
        assert_eq!(world.query::<&Observer>().iter(&world).count(), 1,);

        world
            .entity_mut(entity)
            .trigger(TestEvent)
            .remove::<Observe<TestEvent, ()>>()
            .trigger(TestEvent);

        assert_eq!(
            *world.get_resource::<CheckTriggered>().unwrap(),
            CheckTriggered(5),
        );
        assert_eq!(world.query::<&Observer>().iter(&world).count(), 0);
    }
}
