extern crate coord_2d;
extern crate direction;
extern crate grid_2d;
extern crate rand;
#[macro_use]
extern crate serde;
extern crate grid_search;
extern crate hashbrown;
extern crate line_2d;
extern crate rgb24;
extern crate shadowcast;
extern crate wfc;

mod pathfinding;
mod terrain;
mod vision;
mod world;

use crate::pathfinding::*;
use crate::vision::*;
pub use crate::world::*;
use coord_2d::*;
use direction::*;
use rand::Rng;
use std::time::Duration;

const NPC_VISION_RANGE: usize = 16;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Input {
    Move(CardinalDirection),
}

pub mod input {
    use super::*;
    pub const UP: Input = Input::Move(CardinalDirection::North);
    pub const DOWN: Input = Input::Move(CardinalDirection::South);
    pub const LEFT: Input = Input::Move(CardinalDirection::West);
    pub const RIGHT: Input = Input::Move(CardinalDirection::East);
}

#[derive(Serialize, Deserialize)]
pub struct Gws {
    world: World,
    visible_area: VisibileArea,
    pathfinding: PathfindingContext,
    player_id: EntityId,
    animation: Vec<Animation>,
    turn: Turn,
}

pub struct ToRender<'a> {
    pub world: &'a World,
    pub visible_area: &'a VisibileArea,
    pub player: &'a Entity,
    pub commitment_grid: &'a CommitmentGrid,
}

#[allow(dead_code)]
enum TerrainChoice {
    StringDemo,
    WfcIceCave(Size),
}

const TERRAIN_CHOICE: TerrainChoice = TerrainChoice::WfcIceCave(Size::new_u16(60, 40));
//const TERRAIN_CHOICE: TerrainChoice = TerrainChoice::StringDemo;

#[derive(Clone)]
pub struct BetweenLevels {
    player: PackedEntity,
}

pub enum End {
    ExitLevel(BetweenLevels),
    PlayerDied,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum AnimationState {
    DamageStart {
        id: EntityId,
        direction: CardinalDirection,
    },
    DamageEnd {
        id: EntityId,
    },
}

const DAMAGE_ANIMATION_PERIOD: Duration = Duration::from_millis(250);

impl AnimationState {
    fn update(self, world: &mut World) -> Option<Animation> {
        match self {
            AnimationState::DamageStart { id, direction } => {
                world.set_taking_damage_in_direction(id, Some(direction));
                Some(Animation::new(
                    DAMAGE_ANIMATION_PERIOD,
                    AnimationState::DamageEnd { id },
                ))
            }
            AnimationState::DamageEnd { id } => {
                world.set_taking_damage_in_direction(id, None);
                world.deal_damage(id, 1);
                None
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Turn {
    Player,
    Engine,
}

enum PlayerTurn {
    Done,
    Cancelled,
    Animation(Animation),
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct Animation {
    next_update_in: Duration,
    state: AnimationState,
}

impl Animation {
    pub fn new(next_update_in: Duration, state: AnimationState) -> Self {
        Self {
            next_update_in,
            state,
        }
    }
    fn tick(self, period: Duration, world: &mut World) -> Option<Self> {
        let Animation {
            next_update_in,
            state,
        } = self;
        if period >= next_update_in {
            state.update(world)
        } else {
            Some(Self {
                next_update_in: next_update_in - period,
                state,
            })
        }
    }
    pub fn damage(id: EntityId, direction: CardinalDirection) -> Self {
        Self::new(
            Duration::from_secs(0),
            AnimationState::DamageStart { id, direction },
        )
    }
}

impl Gws {
    pub fn new<R: Rng>(
        between_levels: Option<BetweenLevels>,
        rng: &mut R,
        debug_terrain_string: Option<&str>,
    ) -> Self {
        let terrain::TerrainDescription {
            size,
            player_coord,
            instructions,
        } = match TERRAIN_CHOICE {
            TerrainChoice::StringDemo => terrain::from_str(
                debug_terrain_string.unwrap_or(include_str!("terrain_string.txt")),
            ),
            TerrainChoice::WfcIceCave(size) => terrain::wfc_ice_cave(size, rng),
        };
        let player = match between_levels {
            None => PackedEntity::player(),
            Some(BetweenLevels { player }) => player,
        };
        let mut world = World::new(size);
        for instruction in instructions {
            world.interpret_instruction(instruction);
        }
        let player_id = world.add_entity(player_coord, player);
        let visible_area = VisibileArea::new(size);
        let mut pathfinding = PathfindingContext::new(size);
        pathfinding.update_player_coord(player_coord, &world);
        for &id in world.npc_ids() {
            pathfinding.commit_to_moving_towards_player(id, &world);
        }
        let mut s = Self {
            world,
            visible_area,
            player_id,
            pathfinding,
            animation: Vec::new(),
            turn: Turn::Player,
        };
        s.update_visible_area();
        s
    }

    fn player_turn(&mut self, input: Input) -> PlayerTurn {
        match input {
            Input::Move(direction) => {
                match self
                    .world
                    .move_entity_in_direction(self.player_id, direction)
                {
                    Ok(ApplyAction::Done) => PlayerTurn::Done,
                    Ok(ApplyAction::Animation(animation)) => {
                        PlayerTurn::Animation(animation)
                    }
                    Err(_) => PlayerTurn::Cancelled,
                }
            }
        }
    }

    fn engine_turn(&mut self) {
        for &(id, direction) in self.pathfinding.committed_movements().iter() {
            match self.world.move_entity_in_direction(id, direction) {
                Ok(ApplyAction::Done) => (),
                Ok(ApplyAction::Animation(animation)) => self.animation.push(animation),
                Err(_) => (),
            }
        }
        let player_coord = self.player().coord();
        self.pathfinding
            .update_player_coord(player_coord, &self.world);
        for &id in self.world.npc_ids() {
            let npc_coord = self.world.entities().get(&id).unwrap().coord();
            if self
                .world
                .can_see(npc_coord, player_coord, NPC_VISION_RANGE)
            {
                self.pathfinding
                    .commit_to_moving_towards_player(id, &self.world);
            }
        }
    }

    fn check_end(&self) -> Option<End> {
        let player = self.player();
        if let Some(cell) = self.world.grid().get(player.coord()) {
            for entity in cell.entity_iter(self.world.entities()) {
                if entity.foreground_tile() == Some(ForegroundTile::Stairs) {
                    return Some(End::ExitLevel(BetweenLevels {
                        player: self.world.pack_entity(self.player_id),
                    }));
                }
            }
        }
        if player.hit_points().unwrap().num == 0 {
            return Some(End::PlayerDied);
        }
        None
    }

    pub fn animate(&mut self, period: Duration) {
        if let Some(animation) = self.animation.pop() {
            if let Some(animation) = animation.tick(period, &mut self.world) {
                self.animation.push(animation);
            }
        }
    }

    pub fn tick<I: IntoIterator<Item = Input>, R: Rng>(
        &mut self,
        inputs: I,
        period: Duration,
        rng: &mut R,
    ) -> Option<End> {
        let _ = rng;
        self.animate(period);
        if self.animation.is_empty() {
            if self.turn == Turn::Player {
                let player_turn = if let Some(input) = inputs.into_iter().next() {
                    self.player_turn(input)
                } else {
                    PlayerTurn::Cancelled
                };
                match player_turn {
                    PlayerTurn::Cancelled => (),
                    PlayerTurn::Done => self.turn = Turn::Engine,
                    PlayerTurn::Animation(animation) => {
                        self.turn = Turn::Engine;
                        self.animation.push(animation);
                    }
                }
            }
        }
        if self.animation.is_empty() {
            if self.turn == Turn::Engine {
                self.engine_turn();
                self.turn = Turn::Player;
            }
        }
        self.animate(Duration::from_secs(0));
        self.update_visible_area();
        self.check_end()
    }

    fn player(&self) -> &Entity {
        self.world.entities().get(&self.player_id).unwrap()
    }

    fn update_visible_area(&mut self) {
        self.visible_area.update(self.player().coord(), &self.world);
    }

    pub fn to_render(&self) -> ToRender {
        ToRender {
            world: &self.world,
            visible_area: &self.visible_area,
            player: self.player(),
            commitment_grid: self.pathfinding.commitment_grid(),
        }
    }
}
