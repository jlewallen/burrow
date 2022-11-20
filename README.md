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

### Actions TODO

```dig "NORTH EXIT" to "SOUTH EXIT" for "A NEW AREA"```

```describe (HERE|MYSELF|#held_or_other) AS "..text..."```

```rename (HERE|MYSELF|#held_or_other) AS "..text..."```

~~~```look inside <X>```~~~

~~~```put <X> inside of <Y>```~~~

~~~```take <X> out of <Y>```~~~

```lock <X>```

```unlock <X>```