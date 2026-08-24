# Resolve the configured repo-local rule package directories from a polint
# config file, for callers that must know them before polint runs.
#
# Reads `.polint.toml` on stdin and prints exactly one of:
#
#   status=default                 no `rules.paths` key; polint uses .polint/rules
#   status=paths                   followed by one resolved path per line
#   status=unsupported             followed by `reason=<text>`
#
# String bodies are masked before any structure is read, so every bracket,
# brace, comma, dot and `=` left in the masked document is structure and never
# data. Whatever the mask cannot represent exactly - a multi-line string, an
# escape inside a value that would become a path, an unbalanced array, a key
# shape this parser does not model - is reported as unsupported. Callers must
# treat that as "cannot know" and fall back to their safe behavior rather than
# assuming the default.

function fail(reason) {
    printf "status=unsupported\nreason=%s\n", reason
    bailed = 1
    exit 0
}

function trim(value) {
    gsub(/^[ \t\r\n]+/, "", value)
    gsub(/[ \t\r\n]+$/, "", value)
    return value
}

# Replace every string body with a `\001<index>\001` token, drop comments, and
# keep the remaining bytes verbatim.
function mask_strings(    total, at, ch, ahead, scan, body, escaped, newline, nextch) {
    total = length(doc)
    at = 1
    while (at <= total) {
        ch = substr(doc, at, 1)
        if (ch == "#") {
            newline = index(substr(doc, at), "\n")
            if (newline == 0) {
                at = total + 1
            } else {
                at = at + newline - 1
            }
            continue
        }
        if (ch == "\"" || ch == "'") {
            ahead = substr(doc, at, 3)
            if (ahead == "\"\"\"" || ahead == "'''") {
                fail("multi-line strings are not supported")
            }
            scan = at + 1
            body = ""
            escaped = 0
            while (scan <= total) {
                nextch = substr(doc, scan, 1)
                if (nextch == "\n") {
                    fail("unterminated string")
                }
                if (ch == "\"" && nextch == "\\") {
                    escaped = 1
                    body = body nextch substr(doc, scan + 1, 1)
                    scan = scan + 2
                    continue
                }
                if (nextch == ch) {
                    break
                }
                body = body nextch
                scan++
            }
            if (scan > total) {
                fail("unterminated string")
            }
            strings++
            string_value[strings] = body
            string_escaped[strings] = escaped
            masked = masked SEP strings SEP
            at = scan + 1
            continue
        }
        masked = masked ch
        at++
    }
}

# Resolve a masked token back to its string, refusing values this parser did
# not decode byte for byte.
function string_of(token,    id) {
    if (token !~ ("^" SEP "[0-9]+" SEP "$")) {
        return SEP
    }
    id = substr(token, 2, length(token) - 2) + 0
    if (id < 1 || id > strings) {
        fail("internal string index out of range")
    }
    if (string_escaped[id]) {
        fail("escapes in a rule path are not supported")
    }
    return string_value[id]
}

# Turn the left side of an assignment into a dotted key, resolving quoted key
# segments. Bails out on any segment shape this parser does not model.
function normalize_key(raw,    parts, count, index_, segment, resolved, out) {
    gsub(/[ \t]/, "", raw)
    if (raw == "") {
        fail("empty key")
    }
    count = split(raw, parts, /\./)
    out = ""
    for (index_ = 1; index_ <= count; index_++) {
        segment = parts[index_]
        if (segment ~ /^[A-Za-z0-9_-]+$/) {
            resolved = segment
        } else if (segment ~ ("^" SEP "[0-9]+" SEP "$")) {
            resolved = string_of(segment)
            if (resolved == SEP) {
                fail("unrecognized quoted key")
            }
        } else {
            fail("unrecognized key syntax")
        }
        out = (out == "" ? resolved : out "." resolved)
    }
    return out
}

# Bracket/brace depth of a masked fragment. Negative depth means the fragment
# closes something it never opened, which this parser refuses to guess about.
function depth_of(fragment,    at, total, ch, depth) {
    total = length(fragment)
    depth = 0
    for (at = 1; at <= total; at++) {
        ch = substr(fragment, at, 1)
        if (ch == "[" || ch == "{") {
            depth++
        } else if (ch == "]" || ch == "}") {
            depth--
            if (depth < 0) {
                fail("unbalanced brackets")
            }
        }
    }
    return depth
}

# Split a masked fragment on commas that sit at depth zero.
function split_top_level(fragment, out,    at, total, ch, depth, current, count) {
    total = length(fragment)
    depth = 0
    current = ""
    count = 0
    for (at = 1; at <= total; at++) {
        ch = substr(fragment, at, 1)
        if (ch == "[" || ch == "{") {
            depth++
        } else if (ch == "]" || ch == "}") {
            depth--
        }
        if (ch == "," && depth == 0) {
            out[++count] = current
            current = ""
            continue
        }
        current = current ch
    }
    out[++count] = current
    return count
}

# Decode `[ "a", "b" ]` into `found_paths`, refusing anything that is not a
# plain array of plain strings.
function parse_array(value,    inner, parts, count, index_, item, resolved) {
    value = trim(value)
    if (substr(value, 1, 1) != "[" || substr(value, length(value), 1) != "]") {
        fail("rules.paths is not an array literal")
    }
    inner = substr(value, 2, length(value) - 2)
    count = split_top_level(inner, parts)
    found_count = 0
    for (index_ = 1; index_ <= count; index_++) {
        item = trim(parts[index_])
        if (item == "") {
            if (index_ == count) {
                continue
            }
            fail("empty entry in rules.paths")
        }
        resolved = string_of(item)
        if (resolved == SEP) {
            fail("rules.paths entries must be strings")
        }
        found_paths[++found_count] = resolved
    }
}

# Decode `{ paths = [...] , ... }`, used for `rules = { ... }`.
function parse_inline_table(value,    inner, parts, count, index_, part, eq, key) {
    value = trim(value)
    if (substr(value, 1, 1) != "{" || substr(value, length(value), 1) != "}") {
        fail("rules is not an inline table")
    }
    inner = substr(value, 2, length(value) - 2)
    count = split_top_level(inner, parts)
    for (index_ = 1; index_ <= count; index_++) {
        part = trim(parts[index_])
        if (part == "") {
            continue
        }
        eq = index(part, "=")
        if (eq == 0) {
            fail("unrecognized inline table entry")
        }
        key = normalize_key(substr(part, 1, eq - 1))
        if (key == "paths") {
            record_paths(substr(part, eq + 1))
        }
    }
}

function record_paths(value) {
    if (seen_paths) {
        fail("rules.paths is assigned more than once")
    }
    seen_paths = 1
    parse_array(value)
}

function read_structure(    lines, total, index_, line, table, inner, is_array, eq, key, value, full) {
    total = split(masked, lines, "\n")
    table = ""
    index_ = 1
    while (index_ <= total) {
        line = trim(lines[index_])
        index_++
        if (line == "") {
            continue
        }
        if (substr(line, 1, 1) == "[") {
            is_array = (substr(line, 1, 2) == "[[")
            if (is_array) {
                if (substr(line, length(line) - 1, 2) != "]]") {
                    fail("unrecognized table header")
                }
                inner = substr(line, 3, length(line) - 4)
            } else {
                if (substr(line, length(line), 1) != "]") {
                    fail("unrecognized table header")
                }
                inner = substr(line, 2, length(line) - 2)
            }
            table = normalize_key(inner)
            if (table == "rules") {
                if (is_array) {
                    fail("[[rules]] is not a rules table")
                }
                if (seen_rules_table) {
                    fail("duplicate [rules] table")
                }
                seen_rules_table = 1
            }
            continue
        }
        eq = index(line, "=")
        if (eq == 0) {
            fail("unrecognized line outside a key assignment")
        }
        key = normalize_key(substr(line, 1, eq - 1))
        value = substr(line, eq + 1)
        while (depth_of(value) != 0) {
            if (index_ > total) {
                fail("unterminated value")
            }
            value = value "\n" lines[index_]
            index_++
        }
        full = (table == "" ? key : table "." key)
        if (full == "rules.paths") {
            record_paths(value)
        } else if (full == "rules" && substr(trim(value), 1, 1) == "{") {
            parse_inline_table(value)
        } else if (full ~ /^rules\.paths\./) {
            fail("rules.paths is used as a table")
        }
    }
}

function emit(    index_) {
    if (!seen_paths) {
        print "status=default"
        exit 0
    }
    print "status=paths"
    for (index_ = 1; index_ <= found_count; index_++) {
        print found_paths[index_]
    }
    exit 0
}

BEGIN {
    SEP = sprintf("%c", 1)
    # Masking walks the document a byte at a time, which is instant for real
    # configs (tens of KB) and quadratic well beyond them. polint reads configs
    # up to 1 MB; past this bound the answer is not worth the wait, so callers
    # get "unsupported" and fall back rather than stalling a CI step.
    MAX_BYTES = 131072
    strings = 0
    seen_paths = 0
    seen_rules_table = 0
    found_count = 0
    masked = ""
    doc = ""
    bailed = 0
}

{
    doc = doc $0 "\n"
    if (length(doc) > MAX_BYTES) {
        fail("config is too large to read here")
    }
}

END {
    if (bailed) {
        exit 0
    }
    if (index(doc, SEP) > 0) {
        fail("config contains a control byte the parser cannot mask")
    }
    mask_strings()
    read_structure()
    emit()
}
