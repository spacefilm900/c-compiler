int printf(const char *, ...);
int puts(const char *);
unsigned long strlen(const char *);
typedef unsigned long size_t;
typedef int wchar_t;
typedef unsigned short char16_t;
typedef unsigned int char32_t;
static const char s8[] = u8"Ä€😀\u00e9";
static const char p[] = "aÄ\u20AC";
static const char16_t s16[] = u"Ä€😀\U0001F601";
static const char32_t s32[] = U"Ä€😀\u00e9";
static const wchar_t sw[] = L"Ä€😀$\u0040";
int main(void) {
  printf("%zu %zu %zu %zu %zu\n", sizeof s8, sizeof p, sizeof s16, sizeof s32, sizeof sw);
  for (size_t i = 0; i < sizeof s8; i++) printf("%02x ", (unsigned char)s8[i]);
  puts("");
  for (size_t i = 0; i < sizeof p; i++) printf("%d ", p[i]);
  puts("");
  for (size_t i = 0; i < sizeof s16 / 2; i++) printf("%04x ", s16[i]);
  puts("");
  for (size_t i = 0; i < sizeof s32 / 4; i++) printf("%x ", (unsigned)s32[i]);
  puts("");
  for (size_t i = 0; i < sizeof sw / 4; i++) printf("%x ", (unsigned)sw[i]);
  puts("");
  printf("%d %u %u %d %d\n", L'é', (unsigned)u'€', (unsigned)U'😀', L'\U0010FFFF', '\377');
  printf("%d %d\n", (int)sizeof(L'a'), (int)sizeof(u'a'));
  printf("%zu\n", strlen("ñandú"));
  return 0;
}
