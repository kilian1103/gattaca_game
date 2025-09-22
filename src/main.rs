use std::collections::{HashMap, HashSet};
use std::{env, fs, io};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use rand::prelude::{IndexedRandom, IteratorRandom};
use rayon::prelude::*;
use rand::{rng};
use lasso::{Rodeo, Spur};
use num_cpus;

fn main() {
    // Assumption: If an ant is stuck in a room with no exits, they stay there forever until game ends
    // Assumption: Two or more ants in the same colony, destroy the colony and they all die
    // Assumption: When an ant is stuck in a colony with no exits, it does count as a move to stay
    // Assumption: World map is bidirectional
    // Assumption: World map is well-formed (no self-loops, no duplicate directions in a colony)
    // Assumption: There are only 4 possible directions: north, south, east, west
    // Assumption: Colony names and directions are case-sensitive and contain no spaces and contain no number characters
    
    let data_file_path = "./data/hiveum_map_small.txt";
    let N: usize = env::args().nth(1).expect("Please provide a valid ants size").parse().unwrap();
    println!("Num of ants to spawn: {}", N);
    let num_cpus = num_cpus::get(); // get number of CPUs on this local machine
    // can be set manually for testing purposes to decrease number of threads
    println!("Number of CPUs on this local machine: {}", num_cpus);

    let oppositeDirections: HashMap<&str, &str> = HashMap::from([
        ("north", "south"),
        ("south", "north"),
        ("east", "west"),
        ("west", "east"),
    ]);

    // set num_threads = available num_cpus
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_cpus)
        .build_global()
        .unwrap();
    println!("Number of threads for this program: {}", num_cpus);

    // string interner to save heap alloc memory
    print!("Building world map...");
    let (worldMap, interner) = build_map(data_file_path).unwrap();
    let worldMap = Arc::new(RwLock::new(worldMap)); // share worldMap across threads
    let interner = Arc::new(RwLock::new(interner)); // share interner across threads

    println!("Initializing ant positions...");
    let allColonies: Vec<Spur> = {
        let worldMap = worldMap.read().unwrap();
        worldMap.keys().cloned().collect()
    };
    if allColonies.is_empty() {
        panic!("Error: world map is empty!");
    }
    if N > allColonies.len() {
        println!("Warning: number of ants exceeds number of colonies. Some colonies will have multiple ants.");
    }
    let mut rng = rng();
    let mut antPos: Vec<(usize, Spur)> = (0..N)
        .map(|id| (id,
                   *allColonies
                       .choose(&mut rng)
                       .unwrap())
        )
        .collect();

    println!("Starting simulation...");
    let start_time = Instant::now();
    for i in 0..10_000 {
        // evolve ants - multi-threaded
        let chunk_size = (N / num_cpus).max(1);
        move_ants(&mut antPos, &worldMap, chunk_size);

        // detect collision - single thread
        let mut worldMapWrite = worldMap.write().unwrap();
        let mut interner_write = interner.write().unwrap();
        detect_collision(&mut antPos, &mut worldMapWrite, &mut interner_write,
                         &oppositeDirections);

        if antPos.is_empty() {
            // all ants are dead
            println!("All ants are dead. Simulation ends at iteration {}", i);
            break;
        }
    }
    let duration = start_time.elapsed();
    println!("Simulation ends.");
    
    println!("Remaining colonies....");
    let finalWorldMap = worldMap.read().unwrap();
    if finalWorldMap.is_empty() {
        println!("All colonies have been destroyed.");
        println!("Simulation took {} milli seconds.", duration.as_millis());
        return;
    } else {
        const DIRECTIONS_IN_ORDER: [&str; 4] = ["north", "south", "east", "west"];
        for (colony, exits) in finalWorldMap.iter() {
            let mut interner_write = interner.write().unwrap();
            print!("{} ", interner_write.resolve(colony));
            for &direction in DIRECTIONS_IN_ORDER.iter() {
                let direction_key = interner_write.get_or_intern(direction);
                if let Some(destination) = exits.get(&direction_key) {
                    print!("{}={} ", direction, interner_write.resolve(destination));
                }
            }
            println!();
        }
    }
    println!("Simulation took {} milli seconds.", duration.as_millis());
}

fn move_ants(antPos: &mut Vec<(usize, Spur)>, worldMap: &Arc<RwLock<HashMap<Spur, HashMap<Spur, Spur>>>>, chunk_size: usize)  {
    // evolve ants - multi-threaded
    // chunks of ants per thread
    antPos.par_chunks_mut(chunk_size).for_each(|chunk| {
        let worldMapRead = worldMap.read().unwrap();
        let mut rng = rng();
        // single ant move logic
        for (_id, ant) in chunk.iter_mut() {
            let exits = match worldMapRead.get(ant) {
                Some(exits) => exits,
                None => continue, // no exits, ant is trapped, stay in the same room
            };
            if let Some(newRoom) = exits.values().choose(&mut rng) {
                *ant = *newRoom; // move ant to random new room
            }
        }
    });
}

fn detect_collision(antPos: &mut Vec<(usize,Spur)>, worldMap: &mut HashMap<Spur, HashMap<Spur, Spur>>, interner: &mut Rodeo,
                    oppositeDirections: &HashMap<&str, &str>){

    let mut collisionCounter: HashMap<Spur, Vec<usize>> = HashMap::new();
    let mut deadAnts: HashSet<usize> = HashSet::new();
    let mut doomedColonies: HashSet<Spur> = HashSet::new();
    let mut neighborsTunnelsToDelete: Vec<(Spur, Spur)> = Vec::new();

    // count ants in each colony
    for (id, position) in antPos.iter() {
        collisionCounter.entry(*position).or_default().push(*id);
    }

    // find collisions
    collisionCounter.into_iter().for_each(|(colony, ant_indices)| {
        if ant_indices.len() > 1 {
            //assuming that if there is a collision, all ants (>=2) in that colony die
            println!("{} has been destroyed by ant {} and ant {}!", interner.resolve(&colony), ant_indices[0], ant_indices[1]);
            doomedColonies.insert(colony);
            deadAnts.extend(ant_indices);
        }
    });

    if deadAnts.is_empty() {
        return; // no collisions, return early
    }

    // find tunnels to delete in neighboring colonies
    for &doomed in &doomedColonies {
        if let Some(exits) = worldMap.get(&doomed) {
            for (direction, destination) in exits.iter() {
                if let Some(opposite) = oppositeDirections.get(interner.resolve(direction)) {
                    let opposite_key = interner.get_or_intern(opposite);
                    neighborsTunnelsToDelete.push((*destination, opposite_key));
                }
            }
        }
    }

    // make deletions to the world map by deleting tunnels to doomed colonies
    for (dest, dir) in neighborsTunnelsToDelete {
        if let Some(exits) = worldMap.get_mut(&dest) {
            exits.remove(&dir);
        }
    }
    // remove doomed colonies from worldMap
    for doomed in doomedColonies {
        worldMap.remove(&doomed);
    }

    // remove dead ants from antPos
    antPos.retain(|(id, _position)| !deadAnts.contains(id));

}

fn build_map(map_path: &str) -> io::Result<(HashMap<Spur, HashMap<Spur, Spur>>, Rodeo)> {

    // This will be our main data structure for the entire world.
    let mut world: HashMap<Spur, HashMap<Spur, Spur>> = HashMap::new();
    let mut interner = Rodeo::new();

    // Read the entire file content into a string.
    let file_content = fs::read_to_string(map_path)?;

    // Process the file content line by line.
    for line in file_content.lines() {
        // Trim whitespace from the line. If the line is now empty, skip it.
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Use `split_whitespace` to create an iterator over the parts of the line.
        let mut parts = line.split_whitespace();

        // The first part is the colony's name. If there's no first part (e.g., an empty line),
        // we skip to the next iteration of the loop.
        let colony_name = match parts.next() {
            Some(name) => name.to_string(),
            None => continue,
        };

        // The rest of the items in the `parts` iterator are the connections (e.g., "north=Buzz").
        let mut exits = HashMap::new();
        for connection_str in parts {
            // If the '=' is missing, it returns `None`, and we simply ignore this malformed part.
            if let Some((direction, destination)) = connection_str.split_once('=') {
                // We convert the `&str` parts to `Spur`s to store them in our world map.
                let destination_key = interner.get_or_intern(destination);
                let direction_key = interner.get_or_intern(direction);
                exits.insert(direction_key, destination_key);
            }
        }

        // Insert the parsed exits into the main world map for the current colony.
        let colony_key = interner.get_or_intern(colony_name);
        world.entry(colony_key).or_default().extend(exits);
    }

    Ok((world, interner))
}
