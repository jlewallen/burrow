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

### Actions TODO

1. ```look inside <X>```
2. ```put <X> inside of <Y>```
3. ```take <X> out of <Y>```
4. ```lock <X>```
5. ```unlock <X>```