# slice

A small CLI tool to slice input by lines or bytes using Python-like ranges. It reads from a file or stdin and writes the selected region to stdout. Negative indices count from the end, and you can use relative lengths.

## Usage

```
Usage: slice [OPTIONS] <range> [input]

Args:
  <range>  Range in the form start:end (start/end can be negative). If start is omitted, it defaults to 0. If end is omitted, it defaults to the input length. End may be "+N" to specify a length relative to start.
  [input]  Input file path. Use "-" or omit to read from stdin.

Options:
  -c, --byte   Count by bytes instead of lines (default counts lines)
  -h, --help   Print help
  -V, --version Print version
```

### Range rules
- Format: `start:end`
- Omit `start` to mean `0` (from the beginning)
- Omit `end` to mean the end of input
- Use negative values to count from the end (e.g., `-10:` = last 10)
- Use `+N` for `end` to specify length relative to `start` (e.g., `100:+10`)

### Examples

```bash
# First 5 lines from stdin
seq 1 100 | slice 0:5
```

Output:
```
1
2
3
4
5
```

```bash
# Last 10 lines from stdin
seq 1 100 | slice -10:
```

Output:
```
91
92
93
94
95
96
97
98
99
100
```

```bash
# Ten lines starting at line 50 (file input)
seq 1 200 > numbers.txt
slice 50:+10 numbers.txt
```

Output:
```
51
52
53
54
55
56
57
58
59
60
```

```bash
# Last 1 KiB from stdin (bytes mode)
cat bigfile | slice -c -1024:
```

By default, counting is by lines (newline-delimited). Use `-c/--byte` to switch to byte counting.
