#!/usr/bin/env perl

use strict;
use warnings;

use File::Path qw(make_path);

my $output_dir = 'resources/icons';
my $base_url =
	'https://raw.githubusercontent.com/lucide-icons/lucide/main/icons';

my @icons = (
	'mouse-pointer-2',
	'pen-tool',
	'spline',
	'slash',
	'square',
	'circle',
	'layers',
	'eye',
	'eye-off',
	'plus',
	'trash-2',
	'undo-2',
	'redo-2',
	'zoom-in',
	'zoom-out',
	'hand',
	'save',
	'file-input',
	'file-output',
	'brush',
	'droplet',
	'paint-bucket',
	'eraser',
	'pencil',
	'arrow-up',
	'arrow-down',
	'link-2',
	'circle-off',
	'check',
	'x',
	'refresh-cw',
);

make_path($output_dir);

for my $icon (@icons) {
	my $url = "$base_url/$icon.svg";
	my $output = "$output_dir/$icon.svg";

	print "Downloading $icon.svg...\n";

	my $status = system(
		'curl',
		'--fail',
		'--location',
		'--silent',
		'--show-error',
		'--output',
		$output,
		$url,
	);

	die "Failed to download $url\n"
		if $status != 0;
}

print 'Downloaded '
	. scalar(@icons)
	. " icons.\n";
