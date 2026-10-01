# Print uncovered (DA with count 0) line numbers for each core module,
# plus the LF/LH totals. Usage: awk -f uncovered.awk lcov.info
/^SF:/ {
    file = substr($0, 4)
    gsub(/\\/, "/", file)
    core = 0
    for (m in modules) if (file ~ ("/" m "$")) core = 1
    current = file
    found = 0
    hit = 0
    lines = ""
}
core && /^DA:/ {
    split(substr($0, 4), a, ",")
    if (a[2] == 0) lines = lines " " a[1]
}
/^LF:/ { if (core) found = substr($0, 4) }
/^LH:/ {
    if (core) {
        hit = substr($0, 4)
        printf "[%s] %d/%d lines  uncovered:%s\n", current, hit, found, lines
    }
}
BEGIN {
    modules["ledger/src/engine.rs"] = 1
    modules["ledger/src/balances.rs"] = 1
    modules["ledger/src/money.rs"] = 1
    modules["ledger/src/reconcile.rs"] = 1
}