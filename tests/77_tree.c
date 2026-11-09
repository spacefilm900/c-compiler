int printf(const char *, ...);

struct Tree {
    int key;
    struct Tree *left;
    struct Tree *right;
};

static struct Tree pool[8];
static int used;

struct Tree *node(int key) {
    struct Tree *n = &pool[used];
    used = used + 1;
    n->key = key;
    n->left = 0;
    n->right = 0;
    return n;
}

struct Tree *insert(struct Tree *root, int key) {
    if (!root) {
        return node(key);
    }
    if (key < root->key) {
        root->left = insert(root->left, key);
    } else if (key > root->key) {
        root->right = insert(root->right, key);
    }
    return root;
}

int found(struct Tree *root, int key) {
    while (root) {
        if (key == root->key) {
            return 1;
        }
        if (key < root->key) {
            root = root->left;
        } else {
            root = root->right;
        }
    }
    return 0;
}

int walk(struct Tree *root) {
    if (!root) {
        return 0;
    }
    return walk(root->left) + root->key + walk(root->right);
}

int main(void) {
    struct Tree *root = 0;
    root = insert(root, 4);
    root = insert(root, 2);
    root = insert(root, 6);
    root = insert(root, 1);
    root = insert(root, 3);
    root = insert(root, 5);
    root = insert(root, 7);
    printf("%d\n", walk(root));
    printf("%d %d %d %d\n", found(root, 1), found(root, 5), found(root, 8), found(root, 4));
    printf("%d %d %d\n", root->key, root->left->right->key, root->right->left->key);
    return 0;
}
