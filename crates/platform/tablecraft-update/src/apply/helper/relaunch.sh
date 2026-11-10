# Fixed helper source. Application paths arrive only as positional arguments.
parent=$1
launch=$2
working_directory=$3
backup=$4
kind=$5
waited=0
maximum_wait=300

while kill -0 "$parent" 2>/dev/null; do
    if [ "$waited" -ge "$maximum_wait" ]; then
        exit 1
    fi
    /bin/sleep 1
    waited=$((waited + 1))
done

# A timeout or incomplete apply retains every backup for explicit recovery.
if [ ! -f "$backup/READY" ]; then
    exit 1
fi

if [ "$kind" = bundle ]; then
    /usr/bin/open -n "$launch" || exit 1
else
    cd "$working_directory" || exit 1
    "$launch" </dev/null >/dev/null 2>&1 &
    application=$!
    /bin/sleep 1
    kill -0 "$application" 2>/dev/null || exit 1
fi

# The parent created this exact private directory; never discover cleanup paths.
/bin/rm -rf -- "$backup"
