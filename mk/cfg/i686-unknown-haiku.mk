# i686-unknown-haiku configuration
CROSS_PREFIX_i686-unknown-haiku=i586-pc-haiku-
CC_i686-unknown-haiku=$(CC)
CXX_i686-unknown-haiku=$(CXX)
CPP_i686-unknown-haiku=$(CPP)
AR_i686-unknown-haiku=$(AR)
CFG_LIB_NAME_i686-unknown-haiku=lib$(1).so
CFG_STATIC_LIB_NAME_i686-unknown-haiku=lib$(1).a
CFG_LIB_GLOB_i686-unknown-haiku=lib$(1)-*.so
CFG_LIB_DSYM_GLOB_i686-unknown-haiku=lib$(1)-*.dylib.dSYM
CFG_CFLAGS_i686-unknown-haiku := -m32 $(CFLAGS)
CFG_GCCISH_CFLAGS_i686-unknown-haiku := -Wall -Werror -g -fPIC -m32 $(CFLAGS)
CFG_GCCISH_CXXFLAGS_i686-unknown-haiku := -fno-rtti $(CXXFLAGS)
CFG_GCCISH_LINK_FLAGS_i686-unknown-haiku := -shared -fPIC -ldl -pthread  -lrt -g -m32
CFG_GCCISH_PRE_LIB_FLAGS_i686-unknown-haiku := -Wl,-whole-archive
CFG_GCCISH_POST_LIB_FLAGS_i686-unknown-haiku := -Wl,-no-whole-archive
CFG_DEF_SUFFIX_i686-unknown-haiku := .linux.def
CFG_LLC_FLAGS_i686-unknown-haiku :=
CFG_INSTALL_NAME_i686-unknown-haiku =
CFG_EXE_SUFFIX_i686-unknown-haiku =
CFG_WINDOWSY_i686-unknown-haiku :=
CFG_UNIXY_i686-unknown-haiku := 1
CFG_PATH_MUNGE_i686-unknown-haiku := true
CFG_LDPATH_i686-unknown-haiku :=
CFG_RUN_i686-unknown-haiku=$(2)
CFG_RUN_TARG_i686-unknown-haiku=$(call CFG_RUN_i686-unknown-haiku,,$(2))
CFG_GNU_TRIPLE_i686-unknown-haiku := i686-unknown-haiku
