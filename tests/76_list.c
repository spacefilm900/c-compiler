int printf(const char *, ...);

struct Node {
    int val;
    struct Node *next;
};

int sum(struct Node *p) {
    int total = 0;
    while (p) {
        total += p->val;
        p = p->next;
    }
    return total;
}

int main(void) {
    struct Node nodes[4];
    nodes[0].val = 1;
    nodes[0].next = &nodes[1];
    nodes[1].val = 2;
    nodes[1].next = &nodes[2];
    nodes[2].val = 3;
    nodes[2].next = &nodes[3];
    nodes[3].val = 4;
    nodes[3].next = 0;
    struct Node *p = &nodes[0];
    printf("%d\n", sum(p));
    printf("%d %d %d\n", p->val, p->next->val, p->next->next->next->val);
    printf("%d\n", p->next->next->next->next == 0);
    return 0;
}
