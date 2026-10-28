# search-handles — remove the second result store

Cards contain verified source/revision/representation/location/excerpt. Optional expansion uses the emitted
`(window_id, item_index)` locator in the same live session, not a minted bearer token or separate authority map.

[FUNCTIONS.md](FUNCTIONS.md) owns removal of the old stores. The [window recipe](../search-continuation/FUNCTIONS.md)
alone owns admission, serialization, expiry and output commit; the evidence owner verifies retained ranges.
Do not preserve an exclusive mutable window borrow across reads/writes to reuse old PreparedPage mechanics.
Move useful range reading with consumers, then delete both old handle catalogues and their exclusive fixtures.
