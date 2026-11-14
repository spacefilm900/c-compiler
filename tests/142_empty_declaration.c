/* Empty declarations at file scope (a lone ';'). C11 6.7 does not allow
   them; Clang and GCC accept them. SQLite's shell expands
   SQLITE_EXTENSION_INIT1; to this form. */

;
int x;
;
int main(void) {
    return x;
}
