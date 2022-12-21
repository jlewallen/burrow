### Early Stages Project

For more information, this is a good place to start until I've got time for this README:

https://github.com/jlewallen/dimsum/

## TODO

1. Add entity templates, allow for custom ones.
2. Our tests should render replies.
3. Intermediate grammar.

```
"PUT #held IN #held"
"PUT #held (INSIDE OF|IN) (#held_or_other)?"
"TAKE (OUT)? #contained (OUT OF #held_or_other)?"
"HOLD (#unheld)?"
"DROP #held?"
```

4. Domain events.
5. Improve test name language. Need a more consistent style.
6. Move from lookup_by_key/gid to generalized lookup_by<T>()

### Musings

I'm on the lookout for a better way to organize the mutation of entities and
scopes. There are a few things I'd like to get.

Should be easy to group multiple mutations/operations into one "batch", so that
JSON serializations are minimized. Maybe we should defer final "save"
serializations to the actual entity save?

I had a half formed idea in my head that involved defining modifications on a
trait that pulls the borrows out of the user code.

### Actions TODO

~~~```dig "NORTH EXIT" to "SOUTH EXIT" for "A NEW AREA"```~~~

```describe (HERE|MYSELF|#held_or_other) AS "..text..."```

```rename (HERE|MYSELF|#held_or_other) AS "..text..."```

~~~```look inside <X>```~~~

~~~```put <X> inside of <Y>```~~~

~~~```take <X> out of <Y>```~~~

```make item "A KEY"```

```lock <X>```

```unlock <X>```