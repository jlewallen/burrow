### Burrow Mud Engine

This is a very "Early Stages Project". For more information, this is a good
place to start until I've got time for this README:

https://github.com/jlewallen/dimsum/

## An Idea

A reply can be intercepted, as JSON and augmented via middleware.

General algorithm is to begin with an Evaluable and iterate until things
terminate, giving every plugin a chance to intervene at each step. This means
that the Evaluable enum and the Effect enum need to serialize to and from JSON,
which also means Action's and Reply's. Not sure how I feel about that.

A simple example:

1. Evaluable::Phrase(user, "look") -> Effect::Action(Action)
2. Evaluable::Action(Action) -> Effect::Reply(Reply)

Resolving Surroundings happens when an Action is evaluated. One tricky part is
how to handle this in dynlib. The fewer times we need to cross that boundary,
the better and yet I'd love for the shared library to be able to go from Phrase
to Reply.

A more complicated example:

1. Evaluable::Phrase(user, "go north") -> Effect::Action(Action)
2. Evaluable::Action(Action) -> Effect::Attempted(Move)
3. Evaluable::Attempted(Move) -> Effect::Reply(Reply)

One of the tricky parts is being able to prevent/stop an Action from another
plugin the way we can with Hooks.

This would be hard with a "middleware" approach because Actions actually modify
and make changes to Entities. One solution is to "isolate" each Action and
return the state with the Effect. Then downstream Evaluators could throw them away.

Hmmm.

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
7. Eat/Drink
8. Home/Limbo
9. Freeze/Unfreeze
10. Invite
11. Wear/Remove (Clothing)
12. Make

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

### Performance Profiling

You may need to run this to enable pprof:

```
echo -1 | sudo tee /proc/sys/kernel/perf_event_paranoid
```