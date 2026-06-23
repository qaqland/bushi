#include <stdio.h>

#include "version.h"

int
main(void)
{
	printf("git version: %s\n", git_version_string);
	return 0;
}
