### Burrow Mud Engine

This is a very "Early Stages Project". For more information, this is a good
place to start until I've got time for this README:

https://github.com/jlewallen/dimsum/

### Intermediate Grammar

The idea with this is to allow plugins to supply grammars at a slightly higher
level than nom to simplify things and allow for a convention around them and
parameter binding. For example, it may be possible to take something like:

```
"PUT #held IN #held"
"PUT #held (INSIDE OF|IN) (#held_or_other)?"
"TAKE (OUT)? #contained (OUT OF #held_or_other)?"
"HOLD (#unheld)?"
"DROP #held?"
```

And combine with rust's "magic parameters" pattern to make composing actions easier:

```rust
#[action("PUT #held IN #held")]
pub fn place_inside(Held(item), Held(item)) {
    // Yada yada
}
```

### Action Ideas

```
make item "A KEY"

lock <X>```
unlock <X>

eat <X>
drink <X>

home
limbo

freeze <X>
unfreeze <X>

invite
```

### Performance Profiling

You may need to run this to enable pprof:

```
echo -1 | sudo tee /proc/sys/kernel/perf_event_paranoid
```