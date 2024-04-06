use std::sync::atomic::Ordering;

use crate::search::Arena;

#[test]
fn supports_type_test() {
    let arena: Arena<4> = Arena::new(100_000).unwrap();
    assert!(arena.supports_type::<[u8; 4]>());
    assert!(arena.supports_type::<[u8; 3]>());
    assert!(arena.supports_type::<[u8; 1]>());
    assert!(arena.supports_type::<[u8; 8]>());
    assert!(!arena.supports_type::<usize>());
    let index = arena.add(42u32).unwrap();
    assert_eq!(*arena.get(&index), 42);
}

#[test]
fn slice_test() {
    let arena: Arena<4> = Arena::new(8).unwrap();
    let slice_index = arena.add_slice(vec![1u32, 2, 3].into_iter()).unwrap();
    let index = arena.add([4u32, 5]).unwrap();
    let index2 = arena.add([6u32, 7, 8]).unwrap();
    assert_eq!(arena.get_slice(&slice_index), &[1, 2, 3]);
    assert_eq!(*arena.get(&index), [4, 5]);
    assert_eq!(*arena.get(&index2), [6, 7, 8]);
}

#[test]
fn slot_57_size() {
    let arena: Arena<57> = Arena::new(3).unwrap();
    let index = arena.add([3_u8; 57]).unwrap();
    let slice_index = arena
        .add_slice(vec![[4_u8; 57], [5_u8; 57]].into_iter())
        .unwrap();

    assert_eq!(arena.add([3_u8; 57]), None);
    assert_eq!(arena.add(1_u8), None);
    assert_eq!(arena.get(&index), &[3; 57]);
    assert_eq!(arena.get_slice(&slice_index), &[[4; 57], [5; 57]]);
}

#[test]
fn slot_5_alignment_full() {
    let arena: Arena<5> = Arena::new(5).unwrap();

    // u16 has alignment 2, and is incompatible with an arena of alignment 5
    assert!(!arena.supports_type::<u16>());

    let mut slice_index1 = arena
        .add_slice([[0u8, 1u8, 2u8], [0u8, 1u8, 2u8], [0u8, 1u8, 2u8]].into_iter())
        .unwrap();

    let mut n = arena.add(42u8).unwrap();
    *arena.get_mut(&mut n) += 1;

    let mut slice_index2 = arena
        .add_slice([[0u8, 1u8, 2u8], [0u8, 1u8, 2u8], [0u8, 1u8, 2u8]].into_iter())
        .unwrap();

    let slice1 = arena.get_slice_mut(&mut slice_index1);
    let slice2 = arena.get_slice_mut(&mut slice_index2);

    assert_eq!(arena.add(0u8), None);
    assert_eq!(arena.stats.padding_bytes.load(Ordering::SeqCst), 16);

    // Mutate the slices in the arena. Run with Miri to ensure nothing bad happens
    for a in slice1.iter_mut() {
        for n in a {
            *n += 1;
        }
    }
    for a in slice2.iter_mut() {
        for n in a {
            *n += 1;
        }
    }

    assert_eq!(slice1.iter().flatten().sum::<u8>(), 18);
    assert_eq!(slice2.iter().flatten().sum::<u8>(), 18);
    assert_eq!(*arena.get(&n), 43);
}
