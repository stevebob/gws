use gws::*;
use prototty::*;

pub struct MapView;

const FLOOR: ViewCell = ViewCell::new()
    .with_character('.')
    .with_foreground(colours::WHITE)
    .with_background(colours::BLACK);
const WALL: ViewCell = ViewCell::new()
    .with_character('#')
    .with_foreground(colours::WHITE)
    .with_background(colours::BLACK);
const PLAYER: ViewCell = ViewCell::new()
    .with_character('@')
    .with_bold(true)
    .with_foreground(colours::WHITE)
    .with_background(colours::BLACK);
const TREE: ViewCell = ViewCell::new()
    .with_character('&')
    .with_bold(true)
    .with_foreground(colours::WHITE)
    .with_background(colours::BLACK);
const STAIRS: ViewCell = ViewCell::new()
    .with_character('>')
    .with_bold(true)
    .with_foreground(rgb24(220, 100, 50))
    .with_background(colours::BLACK);

const FLAME: ViewCell = ViewCell::new()
    .with_character('Ψ')
    .with_bold(true)
    .with_foreground(rgb24(255, 120, 0))
    .with_background(colours::BLACK);

const ALTAR: ViewCell = ViewCell::new()
    .with_character('₪')
    .with_bold(true)
    .with_foreground(rgb24(0, 200, 50))
    .with_background(colours::BLACK);

const FOUNTAIN: ViewCell = ViewCell::new()
    .with_character('≈')
    .with_bold(true)
    .with_foreground(rgb24(50, 100, 200))
    .with_background(colours::BLACK);

impl<'a> View<&'a Gws> for MapView {
    fn view<G: ViewGrid, R: ViewTransformRgb24>(
        &mut self,
        game: &'a Gws,
        context: ViewContext<R>,
        grid: &mut G,
    ) {
        let to_render = game.to_render();
        for ((coord, cell), visibility) in to_render
            .world
            .grid()
            .enumerate()
            .zip(to_render.visible_area.iter())
        {
            if !visibility.is_discovered() {
                continue;
            }
            let mut view_cell = match cell.background_tile() {
                BackgroundTile::Floor | BackgroundTile::Ground => FLOOR,
                BackgroundTile::IceWall
                | BackgroundTile::StoneWall
                | BackgroundTile::BrickWall => WALL,
            };
            for entity in cell.entity_iter(to_render.world.entities()) {
                let foreground_view_cell =
                    entity.foreground_tile().and_then(|foreground_tile| {
                        match foreground_tile {
                            ForegroundTile::Bruiser => None,
                            ForegroundTile::Block => None,
                            ForegroundTile::Spike => None,
                            ForegroundTile::HealthPickup => None,
                            ForegroundTile::NaturalSpike => None,
                            ForegroundTile::Caster => None,
                            ForegroundTile::Healer => None,
                            ForegroundTile::Spark => None,
                            ForegroundTile::End => None,
                            ForegroundTile::Blink0 => None,
                            ForegroundTile::Blink1 => None,
                            ForegroundTile::Player => Some(PLAYER),
                            ForegroundTile::Tree => Some(TREE),
                            ForegroundTile::Stairs => Some(STAIRS),
                            ForegroundTile::Flame => Some(FLAME),
                            ForegroundTile::Altar => Some(ALTAR),
                            ForegroundTile::Fountain => Some(FOUNTAIN),
                        }
                    });
                if let Some(foreground_view_cell) = foreground_view_cell {
                    view_cell = foreground_view_cell.coalesce(view_cell);
                    break;
                }
            }
            grid.set_cell_relative(coord, 0, view_cell, context);
        }
    }
}
