use hypolia_towerwars::game::Layout;
use hypolia_towerwars::map::Map;

fn main() {
    let layout = Layout::from_map(&Map::test_map());
    println!(
        "{} blocs, {} chunks",
        layout.blocks().count(),
        layout.chunks().len()
    );
}
