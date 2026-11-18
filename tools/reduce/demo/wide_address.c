/* Reducer demo input for --mode bad-ir: the compiler at commit 0cab101
   wrote IR that LLVM rejects for one of these static initializers, an
   address converted to an integer wider than a pointer (pull request 46,
   bug a). Current compilers reject it with a diagnostic instead. */
typedef unsigned long uptr;

struct node {
    struct node *next;
    int value;
};

static struct node tail_node = { 0, 3 };
static struct node head_node = { &tail_node, 1 };

int counter;
int table[8] = { 1, 1, 2, 3, 5, 8, 13, 21 };
const char *greeting = "hello";
int *middle = &table[4];
uptr counter_address = (uptr)&counter;
__int128 wide_address = (__int128)&counter;
long table_size = sizeof table / sizeof table[0];

static int list_sum(const struct node *node) {
    int sum = 0;
    for (; node; node = node->next)
        sum += node->value;
    return sum;
}

int lookup(int index) {
    if (index < 0 || index >= table_size)
        return -1;
    return table[index];
}

int main(void) {
    counter = list_sum(&head_node);
    return lookup(3) + *middle - counter - 4 + (counter_address == 0) + (wide_address == 0);
}
