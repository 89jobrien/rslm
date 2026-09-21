# rslm toolkit — auto-loaded by toolkit-hook when you cd into this repo

export def "check"       [] { cargo check --workspace }
export def "fmt"         [] { cargo fmt --all }
export def "lint"        [] { mise run lint }
export def "test"        [] { mise run test }
export def "conformance" [] { mise run conformance }
export def "ci"          [] { mise run ci }
export def "clean"       [] { cargo clean }

export def "help" [] {
    scope commands
    | where name =~ "^tk "
    | select name
    | sort-by name
}
