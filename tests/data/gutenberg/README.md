# Project Gutenberg #100 (pinned copy)

`pg100.txt.gz` is a gzip of Project Gutenberg ebook #100, *The Complete
Works of William Shakespeare*, as downloaded on 2026-11-07 from
https://www.gutenberg.org/cache/epub/100/pg100.txt (sha256 of the
uncompressed file:
`eb61e33cab23dbe712ad57091ded6c7292ba91ac3bb2764f51a3223ec0068fe5`).

The AWK volume tests use it as input. `scripts/fetch_awk_volume.sh` places
this copy where the suite's `fetch.sh` looks for a cached download, so CI
does not depend on gutenberg.org being up. `fetch.sh` still checks the
sha256 before using it. The text is public domain in the United States;
the Project Gutenberg licence header and footer are kept unchanged inside
the file.
