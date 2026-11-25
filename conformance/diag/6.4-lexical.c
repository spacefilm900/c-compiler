/* N1570 6.4 lexical elements: constraint and syntax violations */
//== 6.4.4.1 an integer constant too large for any type
unsigned long long x = 18446744073709551616;
//== 6.4.4.1 an invalid suffix
int x = 12lul;
//== 6.4.4.1 a digit 8 in an octal constant
int x = 08;
//== 6.4.4.2 a hexadecimal floating constant without an exponent
double d = 0x1.8;
//== 6.4.4.4 an empty character constant
int c = '';
//== 6.4.4.4 an octal escape out of range for unsigned char
int c = '\400';
//== 6.4.4.4 a hexadecimal escape out of range for unsigned char
int c = '\x100';
//== 6.4.4.4 a hexadecimal escape with no digits
int c = '\x';
//== 6.4.3 a universal character name for a basic character
int \u0041bc;
//== 6.4.3 a universal character name for a surrogate
int a\ud800;
//== 6.4.3 a universal character name that is not valid in an identifier (D.1)
int a\u00d7;
//== 6.4.5 a universal character name out of range in a string
const char *s = "\U00110000";
//== 6.4.5 concatenating differently prefixed wide strings
const void *s = u"a" U"b";
//== 6.4.8 a pp-number that is not a valid constant
int x = 0xe+1;
//== 6.4.8 another invalid pp-number
int x = 1.2.3;
//== 6.4.9 an unterminated comment
int x; /* never closed
//== 6.4 an unterminated string literal
const char *s = "abc;
//== 6.4p3 a stray character
int x = 1 @ 2;
//== 6.4.1 a keyword as an identifier
int _Bool = 1;
