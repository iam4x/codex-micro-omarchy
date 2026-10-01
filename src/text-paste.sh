set -eu

# Serialize Micro pastes so a second binding cannot replace an in-flight snippet.
exec 9>"${XDG_RUNTIME_DIR:?}/work-louder-text.lock"
flock -x 9
# Do not let the selection owner inherit and retain the lock.
wl-copy --primary --type 'text/plain;charset=utf-8' -- "$1" 9>&-
wtype -s 100 -M shift -k Insert -m shift -s 500
if [ "$2" = true ]; then
    wtype -k Return
fi
