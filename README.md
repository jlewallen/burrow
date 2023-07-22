### Burrow Mud Engine

This is a very "Early Stages Project". For more information, this is a good
place to start until I've got time for this README:

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