outer() {
    local x=1

    inner() {
        local y=2
        echo "inner: x=$x y=$y"
    }

    inner
    echo "outer: x=$x"
}

make_counter() {
    local count=0
    increment() {
        (( count++ ))
        echo $count
    }
}

outer
make_counter
increment
