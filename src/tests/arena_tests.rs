use crate::search::Arena;

#[test]
fn supports_type_test() {
    let arena = Arena::new(100_000, 4).unwrap();
    assert!(arena.supports_type::<[u8; 4]>());
    assert!(!arena.supports_type::<[u8; 3]>());
    assert!(!arena.supports_type::<[u8; 1]>());
    assert!(arena.supports_type::<[u8; 8]>());
    assert!(!arena.supports_type::<usize>());
    let index = arena.add(42u32);
    assert_eq!(*arena.get(&index), 42);
}
