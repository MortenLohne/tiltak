use crate::search::Arena;

#[test]
fn double_borrow() {
    let arena = Arena::new(100_000, 32).unwrap();
    let mut index = arena.add([0u8; 32]);
    let value = arena.get_mut(&mut index);
    assert_eq!(*value, *arena.get_mut(&mut index));
}
