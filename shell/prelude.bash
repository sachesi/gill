# Runs one command for gill in a fresh, non-interactive bash: restore the state the
# previous command saved, run the command at top level, save the state again.
#
# Arguments: the snapshot to restore, the directory to save into, the bashrc to source
# before the command or an empty word, the variable names fish takes (space-separated),
# then `line LINE` or `argv COMMAND [ARG...]`. Everything here calls builtins through
# `builtin`, so functions the user defines with the same names do not change it.

__gill_snapshot=$1 __gill_dir=$2 __gill_bashrc=$3 __gill_import_names=" $4 "
if [[ $5 == line ]]; then
    __gill_line=$6
else
    # Quoted by bash itself, so the words reach the command unchanged, while an alias
    # named by the first word still expands.
    builtin shift 5
    builtin printf -v __gill_line '%q ' "$@"
fi
builtin set --

builtin shopt -s expand_aliases

# Whatever exists before the snapshot is restored belongs to bash itself or came from
# fish's environment, and stays out of the next snapshot.
builtin declare -A __gill_known
while IFS= builtin read -r __gill_entry; do
    __gill_known[$__gill_entry]=1
done < <(builtin compgen -v; builtin shopt -p; builtin set +o)
# Bash sets this inside the save function itself.
__gill_known[FUNCNAME]=1

__gill_save() {
    builtin trap - EXIT
    builtin local __gill_name

    # Shell options first, so that extglob and the like are on before the functions
    # that need them are parsed. Functions come before aliases: their bodies are
    # stored with aliases already expanded and must not be expanded twice.
    # errexit, nounset, verbose and xtrace are only switched on right before the next
    # command, so they neither stop the restore nor trace it.
    {
        while IFS= builtin read -r __gill_name; do
            [[ -v __gill_known[$__gill_name] ]] || builtin printf '%s\n' "$__gill_name"
        done < <(builtin shopt -p; builtin set +o)

        while IFS= builtin read -r __gill_name; do
            [[ $__gill_name == __gill_* || -v __gill_known[$__gill_name] ]] && continue
            # Exported variables belong to fish; restoring them would undo changes made
            # there since.
            [[ ${!__gill_name@a} == *x* ]] && continue
            builtin printf 'builtin declare -p %s >/dev/null 2>&1 ||\n' "$__gill_name"
            builtin declare -p -- "$__gill_name"
        done < <(builtin compgen -v)

        while IFS= builtin read -r __gill_name; do
            [[ $__gill_name == __gill_* ]] || builtin declare -f -- "$__gill_name"
        done < <(builtin compgen -A function)

        builtin alias -p

        builtin printf '__gill_options=%s\n' "${__gill_flags//[!euvx]/}"
    } >|"$__gill_dir/snapshot.next" || builtin return "$__gill_status"

    {
        builtin printf '%s\0' "$PWD"
        while IFS= builtin read -r __gill_name; do
            [[ ${!__gill_name@a} == *x* && -v $__gill_name ]] || continue
            # Bash never puts arrays in the environment.
            [[ ${!__gill_name@a} == *[aA]* ]] && continue
            builtin printf '%s\0%s\0' "$__gill_name" "${!__gill_name}"
        done < <(builtin compgen -e)
        builtin printf '\0'

        # The functions and aliases fish can call through gill.
        while IFS= builtin read -r __gill_name; do
            [[ $__gill_name == __gill_* ]] || builtin printf '%s\0' "$__gill_name"
        done < <(builtin compgen -A function; builtin compgen -a)
        builtin printf '\0'
    } >|"$__gill_dir/environ.next"

    builtin return "$__gill_status"
}

# Both the trap, which covers `exit`, and the end of this script, which covers a
# command that replaced the trap, save the same way: record `$?` and the options while
# they are still the command's, silently, then turn off the options that would trace
# the save or end it early.
builtin trap -- '{ __gill_status=$? __gill_flags=$-; builtin set +euvx; } 2>/dev/null; __gill_save' EXIT

__gill_options=
if [[ -f $__gill_snapshot ]]; then
    builtin source -- "$__gill_snapshot"
fi

# Before the options are restored, so an errexit from the snapshot cannot stop the import.
if [[ -n $__gill_bashrc ]]; then
    builtin declare -A __gill_before
    while IFS= builtin read -r __gill_name; do
        __gill_before[$__gill_name]=${!__gill_name}
    done < <(builtin compgen -e)

    builtin source -- "$__gill_bashrc"

    # Exports from bashrc stay on the bash side only, and variables fish exported keep
    # fish's values. Named variables are the exception.
    for __gill_name in "${!__gill_before[@]}"; do
        [[ $__gill_import_names == *" $__gill_name "* ]] && continue
        builtin printf -v "$__gill_name" '%s' "${__gill_before[$__gill_name]}"
        builtin export -- "$__gill_name"
    done
    while IFS= builtin read -r __gill_name; do
        [[ $__gill_import_names == *" $__gill_name "* || -v __gill_before[$__gill_name] ]] && continue
        builtin export -n -- "$__gill_name"
    done < <(builtin compgen -e)
fi

for __gill_name in $__gill_import_names; do
    [[ -v "$__gill_name" ]] && builtin export -- "$__gill_name"
done

[[ -z $__gill_options ]] || builtin set "-$__gill_options"
builtin eval -- "$__gill_line"
{ __gill_status=$? __gill_flags=$-; builtin set +euvx; } 2>/dev/null
__gill_save
