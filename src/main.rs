use rand::prelude::*;
use rand::distributions::{Distribution, Standard};
//use lazy_static::lazy_static;
use std::cell::Cell;
use std::thread::LocalKey;
use std::time::{Duration, Instant};

type Time = u32;
const ENTITY_COUNT: usize = 1_000_000;
const CHUNK_SIZE: usize = ENTITY_COUNT * 4;
const ROUNDS: usize = 10;
const TIME_MARGIN: usize = (u8::max_value() as usize * CHUNK_SIZE * 2); // times 2 for saftey
const TIME_CAP: Time = Time::max_value() - TIME_MARGIN as Time;
const PREGENED_RANDOM_U32: usize = CHUNK_SIZE * ROUNDS * 2; // times 3 for saftey
const NUM_BUCKETS: usize = 256;

struct PregenedRand<N> {
    ints: Vec<N>,
    current: Cell<usize>,
}

impl<N> PregenedRand<N>
where
    Standard: Distribution<N>,
    N: Clone + Copy
{
    fn new(num: usize) -> Self {
        PregenedRand {
            ints: (0..num).into_iter().map(|_| random()).collect(),
            current: Cell::new(0),
        }
    }

    #[inline]
    fn next_(&self) -> N {
        let c = self.current.get();
        self.current.set(c + 1);
        assert!(c < self.ints.len(), "Rand overflow");
        self.ints[c]
    }

    #[inline]
    fn next(r: &'static LocalKey<PregenedRand<N>>) -> N {
        r.with(|r| r.next_())
    }

    #[inline]
    fn reset_(&self) {
        self.current.set(0);
    }

    #[inline]
    fn reset(r: &'static LocalKey<PregenedRand<N>>) {
        r.with(|r| r.reset_())
    }
}

thread_local! {
    static U8RAND: PregenedRand<u8> = PregenedRand::new(PREGENED_RANDOM_U32);
}

#[derive(Debug)]
struct Entity {
    time: Time,
}

impl Entity {
    fn new() -> Self {
        Entity {
            time: PregenedRand::next(&U8RAND) as Time,
        }
    }
}

fn make_order(b: usize, bt: usize, e: &[Entity]) -> Vec<(usize, Time)> {
    let mut ret = e
        .iter()
        .enumerate()
        .filter_map(|(i, e)| if e.time as usize % bt == b { Some((i, e.time)) } else { None } )
        .collect::<Vec<_>>();
    ret.sort_unstable_by(|a, b| b.1.cmp(&a.1));
    ret
}

fn do_turns(num_buckets: usize) -> Duration {
    PregenedRand::reset(&U8RAND);

    let mut entities = vec![];
    for _ in 0..ENTITY_COUNT {
        entities.push(Entity::new());
    }

    let mut orders = vec![];
    for i in 0..num_buckets {
        orders.push(make_order(i, num_buckets, &entities));
    }

    let now = Instant::now();
    println!("Started");
    for i in 0..ROUNDS {
        println!("Round {}", i);
        for _ in 0..CHUNK_SIZE {
            let mut chosen = (0, Time::max_value());
            for (i, order) in (&orders).iter().enumerate() {
                let v = order.last().map(|x| x.1).unwrap_or(Time::max_value());
                if v <= chosen.1 {
                    chosen = (i, v);
                }
            }

            let cur = orders[chosen.0].pop().unwrap();
            entities[cur.0].time += PregenedRand::next(&U8RAND) as Time;

            let new_elem = (cur.0, entities[cur.0].time);
            let i = new_elem.1 as usize % num_buckets;
            let pos = orders[i]
                .binary_search_by(|a| new_elem.1.cmp(&a.1))
                .unwrap_or_else(|o| o);
            orders[i].insert(pos, new_elem);
        }

        println!("Late Round {}", i);
        let max = entities.iter().max_by(|a, b| a.time.cmp(&b.time)).unwrap().time;
        if max > TIME_CAP {
            println!("Rebase! {}", i);
            let min = entities.iter().min_by(|a, b| a.time.cmp(&b.time)).unwrap().time;
            entities.iter_mut().for_each(|e| e.time -= min);

            for order in &mut orders {
                order.iter_mut().for_each(|o| o.1 -= min);
            }

            let max = entities.iter().max_by(|a, b| a.time.cmp(&b.time)).unwrap().time;
            if max > TIME_CAP {
                panic!("Spread too large.");
            }
        }
    }
    now.elapsed()
}

fn main() {
    println!("Margin {}, Cap {}, Max {}", TIME_MARGIN, TIME_CAP, Time::max_value());
    println!("{} Entities, {} Actions", ENTITY_COUNT, ROUNDS * CHUNK_SIZE);
    assert!(TIME_MARGIN < TIME_CAP as usize);

    for i in &[256] {
        println!("{} took {:?}", i, do_turns(*i));
    }
}