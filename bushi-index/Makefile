GIT_URL ?= https://github.com/git/git.git
GIT_VER ?= v2.54.0

export NO_CURL    = YesPlease
export NO_EXPAT   = YesPlease
export NO_GETTEXT = YesPlease
export NO_GITWEB  = YesPlease
export NO_ICONV   = YesPlease
export NO_OPENSSL = YesPlease
export NO_PERL    = YesPlease
export NO_PYTHON  = YesPlease
export NO_REGEX   = NeedsStartEnd
export NO_TCLTK   = YesPlease

all: libgit

git:
	git clone --depth 1 --branch $(GIT_VER) $(GIT_URL) git

git/libgit.a: git
	$(MAKE) -C git libgit.a

libgit: git/libgit.a

update-git:
	rm -rf git && git clone --depth 1 --branch $(GIT_VER) $(GIT_URL) git

clean:
	$(MAKE) -C git clean

.PHONY: all libgit update-git clean

