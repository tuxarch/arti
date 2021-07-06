#!/bin/bash

set -e

# A list of the licenses that we currently allow in our code.
#
# If a package supports multiple licenses (using OR), then we are okay
# if it supports _any_ of these licenses.
RECOGNIZED_LICENSES=(
    Apache-2.0
    BSD-2-Clause
    BSD-3-Clause
    CC0-1.0
    ISC
    MIT
    Unlicense
)

# List of packages that don't list a license.
NO_LICENSE=(
    cargo-husky
    fuchsia-cprng
)

containsElement () {
  local e match="$1"
  shift
  for e; do
      [[ "$e" == "$match" ]] && return 0;
  done
  return 1
}

if ! cargo license --version >/dev/null 2>/dev/null; then
    echo "cargo-license is not installed!"
    echo
    echo "For reasonable results, run:"
    echo "    cargo install cargo-license"
    exit 2
fi

cd "$(dirname "$0")/.."

problems=0
IFS=$'\n'
for line in $(cargo license --all-features -t); do
    package=$(echo "$line" | cut -f1)
    licenses=$(echo "$line" | cut -f5)

    # skip the first line.
    if test "$package" = "name" && test "$licenses" = "license"; then
	continue;
    fi
    if test -z "$licenses"; then
	if ! containsElement "$package" "${NO_LICENSE[@]}"; then
	    echo "$package has no license"
	    problems=1
	fi
	continue
    fi

    found_ok=0
    for lic in ${licenses// OR /$'\n'}; do
	if containsElement "$lic" "${RECOGNIZED_LICENSES[@]}"; then
	    found_ok=1
	    break
	fi
    done
    if test $found_ok = "0"; then
	echo "$package does not advertise any supported license!"
	echo "   ($package: $licenses)"
	problems=1
    fi
done

if test "$problems" = 1; then
    echo "You can suppress the above warnings by editing $0..."
    echo "but only do so if we are actually okay with all the licenses!"
fi

exit "$problems"
