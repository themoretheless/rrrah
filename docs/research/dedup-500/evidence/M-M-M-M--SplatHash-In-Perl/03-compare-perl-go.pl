#!/usr/bin/env perl
use strict ;
use warnings ;

use Cwd        qw(abs_path) ;
use File::Path qw(make_path) ;
use File::Spec ;
use File::Temp   qw(tempdir) ;
use Getopt::Long qw(GetOptions) ;
use HTTP::Tiny ;
use Imager ;
use SplatHash   qw(encode_file encode_raw decode) ;
use Time::HiRes qw(time) ;

my $REFERENCE_MODULE  = 'github.com/junevm/splathash/src/go' ;
my $REFERENCE_VERSION = 'v0.0.0-20260228095933-038b4f7025ec' ;
my $REFERENCE_COMMIT  = '038b4f7025ec044ed9f319371e0ec2ed863a6f45' ;
my $LOCAL_ASSETS      = 'assets' ;

my @ASSET_NAMES = qw(
  wallhaven-3q3j6y.jpg
  wallhaven-8525yy.jpg
  wallhaven-gw5dq3.jpg
  wallhaven-gwpgv3.jpg
  wallhaven-ogy2m7.jpg
  wallhaven-ogy7ql.jpg
  wallhaven-q2yx9l.jpg
  wallhaven-w513lr.jpg
  wallhaven-yq5ejd_3840x2160.png
) ;

my $assets_dir        = $LOCAL_ASSETS ;
my $encode_iterations = 10 ;
my $decode_iterations = 1000 ;
my $format            = 'text' ;
my $iterations ;
my ( $download_assets, $include_full, $help ) ;

GetOptions(
  'assets=s'            => \$assets_dir,
  'encode-iterations=i' => \$encode_iterations,
  'decode-iterations=i' => \$decode_iterations,
  'iterations=i'        => \$iterations,
  'format=s'            => \$format,
  'include-full'        => \$include_full,
  'download-assets'     => \$download_assets,
  'help|h'              => \$help,
) or _usage(2) ;

_usage(0) if $help ;
if ( defined $iterations ) {
  die "--iterations must be a positive integer\n" if $iterations < 1 ;
  $encode_iterations = $iterations ;
  $decode_iterations = $iterations ;
}
die "--encode-iterations must be a positive integer\n"
  if $encode_iterations < 1 ;
die "--decode-iterations must be a positive integer\n"
  if $decode_iterations < 1 ;
die "--format must be 'text' or 'markdown'\n"
  if $format ne 'text' && $format ne 'markdown' ;

if ($download_assets) {
  _download_assets($assets_dir) ;
}

my @images = _asset_paths($assets_dir) ;
die "No benchmark images found in '$assets_dir'; use --download-assets or --assets PATH\n"
  if !@images ;

my @perl_results
  = _benchmark_perl(
  \@images,
  $encode_iterations,
  $decode_iterations,
  $include_full,
  ) ;
my @go_results
  = _benchmark_go(
  \@images,
  $encode_iterations,
  $decode_iterations,
  $include_full,
  ) ;

if ( $format eq 'markdown' ) {
  _print_markdown_results(
    \@perl_results,
    \@go_results,
    $encode_iterations,
    $decode_iterations,
  ) ;
} else {
  _print_text_results(
    \@perl_results,
    \@go_results,
    $encode_iterations,
    $decode_iterations,
  ) ;
}

exit 0 ;

sub _benchmark_perl {
  my ( $paths, $encode_count, $decode_count, $with_full ) = @_ ;
  my @results ;

  for my $path ( @{$paths} ) {
    my $name = _basename($path) ;
    my ( $rgba, $width, $height ) = _read_rgba($path) ;
    my $hash = encode_raw( $rgba, $width, $height ) ;

    push @results, [ 'perl', 'encode', $name,
      _milliseconds_per_operation(
        $encode_count,
        sub { encode_raw( $rgba, $width, $height )  },
      )
    ] ;

    push @results, [ 'perl', 'decode', $name,
      _milliseconds_per_operation(
        $decode_count,
        sub { decode($hash)  },
      )
    ] ;

    if ($with_full) {
      push @results, [ 'perl', 'full', $name,
        _milliseconds_per_operation(
          $encode_count,
          sub { encode_file($path)  },
        )
      ] ;
    }
  }

  return @results ;
}

sub _benchmark_go {
  my ( $paths, $encode_count, $decode_count, $with_full ) = @_ ;
  my $go = _find_executable('go') ;
  die "Go is required for comparison benchmarks\n" if !defined $go ;

  my $workdir = tempdir( CLEANUP => 1 ) ;
  _write_file(
    File::Spec->catfile( $workdir, 'go.mod' ),
    "module splathashbench\n\ngo 1.21\n\nrequire $REFERENCE_MODULE $REFERENCE_VERSION\n",
  ) ;
  _write_file( File::Spec->catfile( $workdir, 'main.go' ), _go_runner_source() ) ;

  my @absolute_paths = map { abs_path($_) } @{$paths} ;
  my @command        = (
    $go,                   'run', '-mod=mod', '.',
    '--encode-iterations', $encode_count,
    '--decode-iterations', $decode_count,
    ( $with_full ? '--include-full' : () ),
    @absolute_paths,
  ) ;
  my $output = _run_capture( \@command, $workdir ) ;
  my @results ;

  for my $line ( split /\n/, $output ) {
    next if $line eq q{} ;
    my ( $runtime, $mode, $name, $milliseconds ) = split /\t/, $line ;
    push @results, [ $runtime, $mode, $name, $milliseconds + 0 ] ;
  }

  return @results ;
}

sub _milliseconds_per_operation {
  my ( $count, $code ) = @_ ;

  $code->() ;
  my $start = time ;
  for ( 1 .. $count ) {
    $code->() ;
  }
  my $elapsed = time - $start ;

  return ( $elapsed * 1000 ) / $count ;
}

sub _read_rgba {
  my ($path) = @_ ;
  my $image = Imager->new ;
  $image->read( file => $path )
    or die "Cannot read image '$path': " . $image->errstr . "\n" ;

  if ( $image->getchannels < 4 ) {
    $image = $image->convert( preset => 'addalpha' )
      or die "Cannot add alpha channel to '$path': " . $image->errstr . "\n" ;
  }

  my $width  = $image->getwidth ;
  my $height = $image->getheight ;
  my $rgba   = q{} ;

  for my $y ( 0 .. $height - 1 ) {
    $rgba .= $image->getsamples(
      y        => $y,
      channels => [ 0, 1, 2, 3 ],
    ) ;
  }

  return ( $rgba, $width, $height ) ;
}

sub _download_assets {
  my ($dir) = @_ ;
  make_path($dir) if !-d $dir ;

  my $client = HTTP::Tiny->new ;
  for my $name (@ASSET_NAMES) {
    my $path = File::Spec->catfile( $dir, $name ) ;
    next if -f $path ;

    my $url
      = "https://raw.githubusercontent.com/junevm/splathash/$REFERENCE_COMMIT/assets/$name" ;
    print STDERR "Downloading $name\n" ;
    my $response = $client->get($url) ;
    die "Cannot download '$url': $response->{status} $response->{reason}\n"
      if !$response->{success} ;
    _write_file( $path, $response->{content} ) ;
  }

  return ;
}

sub _asset_paths {
  my ($dir) = @_ ;
  return if !defined $dir || !-d $dir ;

  opendir my $dh, $dir
    or die "Cannot open assets directory '$dir': $!\n" ;
  my @paths = sort map { File::Spec->catfile( $dir, $_ ) }
    grep {/\.(?:jpe?g|png)\z/i} readdir $dh ;
  closedir $dh or die "Cannot close assets directory '$dir': $!\n" ;

  return @paths ;
}

sub _print_text_results {
  my ( $perl_results, $go_results, $encode_count, $decode_count ) = @_ ;
  my %go_by_key = map { $_->[1] . "\t" . $_->[2] => $_->[3] } @{$go_results} ;
  my ( %perl_total, %go_total, %mode_count ) ;

  printf "encode iterations: %d\n", $encode_count ;
  printf "decode iterations: %d\n", $decode_count ;
  printf "%-8s %-8s %-34s %12s %12s %10s\n",
    'mode', 'runtime', 'image', 'ms/op', 'go ms/op', 'ratio' ;

  for my $result ( @{$perl_results} ) {
    my ( $runtime, $mode, $name, $milliseconds ) = @{$result} ;
    my $go_ms = $go_by_key{"$mode\t$name"} ;
    my $ratio = defined $go_ms && $go_ms > 0 ? $milliseconds / $go_ms : 0 ;
    $perl_total{$mode} += $milliseconds ;
    $go_total{$mode}   += $go_ms // 0 ;
    $mode_count{$mode}++ ;
    printf "%-8s %-8s %-34s %12.3f %12.3f %10.2f\n",
      $mode, $runtime, $name, $milliseconds, $go_ms // 0, $ratio ;
  }

  print "\naggregate average:\n" ;
  printf "%-8s %12s %12s %10s\n", 'mode', 'perl ms/op', 'go ms/op', 'ratio' ;
  for my $mode (qw(encode decode full)) {
    next if !$mode_count{$mode} ;
    my $perl_average = $perl_total{$mode} / $mode_count{$mode} ;
    my $go_average   = $go_total{$mode} / $mode_count{$mode} ;
    my $ratio        = $go_average > 0 ? $perl_average / $go_average : 0 ;
    printf "%-8s %12.3f %12.3f %10.2f\n",
      $mode, $perl_average, $go_average, $ratio ;
  }

  return ;
}

sub _print_markdown_results {
  my ( $perl_results, $go_results, $encode_count, $decode_count ) = @_ ;
  my %perl_by_key
    = map { $_->[1] . "\t" . $_->[2] => $_->[3] } @{$perl_results} ;
  my %go_by_key
    = map { $_->[1] . "\t" . $_->[2] => $_->[3] } @{$go_results} ;
  my @names = map { $_->[2] }
    grep { $_->[1] eq 'encode' } @{$perl_results} ;
  my ( $perl_encode_total, $go_encode_total ) = ( 0, 0 ) ;
  my ( $perl_decode_total, $go_decode_total ) = ( 0, 0 ) ;

  print "Encode iterations: $encode_count; decode iterations: $decode_count.\n\n" ;
  print
    "| Image | Perl encode (ms/op) | Go encode (ms/op) | Encode ratio | Perl decode (ms/op) | Go decode (ms/op) | Decode ratio |\n" ;
  print
    "|---|---:|---:|---:|---:|---:|---:|\n" ;

  for my $name (@names) {
    my $perl_encode = $perl_by_key{"encode\t$name"} ;
    my $go_encode   = $go_by_key{"encode\t$name"} ;
    my $perl_decode = $perl_by_key{"decode\t$name"} ;
    my $go_decode   = $go_by_key{"decode\t$name"} ;
    my ( $perl_encode_text, $go_encode_text )
      = _markdown_best_pair( $perl_encode, $go_encode ) ;
    my ( $perl_decode_text, $go_decode_text )
      = _markdown_best_pair( $perl_decode, $go_decode ) ;
    $perl_encode_total += $perl_encode ;
    $go_encode_total   += $go_encode ;
    $perl_decode_total += $perl_decode ;
    $go_decode_total   += $go_decode ;
    printf "| %s | %s | %s | %.2fx | %s | %s | %.2fx |\n",
      $name,
      $perl_encode_text,
      $go_encode_text,
      $perl_encode / $go_encode,
      $perl_decode_text,
      $go_decode_text,
      $perl_decode / $go_decode ;
  }

  my $count               = @names ;
  my $perl_encode_average = $perl_encode_total / $count ;
  my $go_encode_average   = $go_encode_total / $count ;
  my $perl_decode_average = $perl_decode_total / $count ;
  my $go_decode_average   = $go_decode_total / $count ;
  my ( $perl_encode_text, $go_encode_text )
    = _markdown_best_pair( $perl_encode_average, $go_encode_average ) ;
  my ( $perl_decode_text, $go_decode_text )
    = _markdown_best_pair( $perl_decode_average, $go_decode_average ) ;
  printf "| **Average** | %s | %s | %.2fx | %s | %s | %.2fx |\n",
    $perl_encode_text,
    $go_encode_text,
    $perl_encode_average / $go_encode_average,
    $perl_decode_text,
    $go_decode_text,
    $perl_decode_average / $go_decode_average ;

  return ;
}

sub _markdown_best_pair {
  my ( $left, $right ) = @_ ;
  my $left_text  = sprintf '%.3f', $left ;
  my $right_text = sprintf '%.3f', $right ;

  if ( $left < $right ) {
    $left_text = "**$left_text**" ;
  } elsif ( $right < $left ) {
    $right_text = "**$right_text**" ;
  } else {
    $left_text  = "**$left_text**" ;
    $right_text = "**$right_text**" ;
  }

  return ( $left_text, $right_text ) ;
}

sub _go_runner_source {
  return <<'GO';
package main

import (
	"fmt"
	"image"
	"image/jpeg"
	"image/png"
	"os"
	"path/filepath"
	"strings"
	"time"

	splathash "github.com/junevm/splathash/src/go"
)

func main() {
	encodeIterations := 10
	decodeIterations := 1000
	includeFull := false
	args := os.Args[1:]
	for len(args) >= 2 {
		switch args[0] {
		case "--encode-iterations":
			fmt.Sscanf(args[1], "%d", &encodeIterations)
			args = args[2:]
		case "--decode-iterations":
			fmt.Sscanf(args[1], "%d", &decodeIterations)
			args = args[2:]
		case "--include-full":
			includeFull = true
			args = args[1:]
		default:
			goto benchmark
		}
	}
benchmark:
	if encodeIterations < 1 {
		encodeIterations = 1
	}
	if decodeIterations < 1 {
		decodeIterations = 1
	}
	for _, path := range args {
		img, err := loadImage(path)
		if err != nil {
			fmt.Fprintf(os.Stderr, "load %s: %v\n", path, err)
			os.Exit(1)
		}
		name := filepath.Base(path)
		hash := splathash.EncodeImage(img)
		fmt.Printf("go\tencode\t%s\t%.6f\n", name, msPerOp(encodeIterations, func() { splathash.EncodeImage(img) }))
		fmt.Printf("go\tdecode\t%s\t%.6f\n", name, msPerOp(decodeIterations, func() { _, _ = splathash.DecodeImage(hash) }))
		if includeFull {
			fmt.Printf("go\tfull\t%s\t%.6f\n", name, msPerOp(encodeIterations, func() {
				fullImg, err := loadImage(path)
				if err != nil {
					panic(err)
				}
				splathash.EncodeImage(fullImg)
			}))
		}
	}
}

func msPerOp(iterations int, fn func()) float64 {
	fn()
	start := time.Now()
	for i := 0; i < iterations; i++ {
		fn()
	}
	return float64(time.Since(start).Microseconds()) / 1000.0 / float64(iterations)
}

func loadImage(path string) (image.Image, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()
	switch strings.ToLower(filepath.Ext(path)) {
	case ".jpg", ".jpeg":
		return jpeg.Decode(f)
	case ".png":
		return png.Decode(f)
	default:
		return nil, fmt.Errorf("unsupported image extension")
	}
}
GO
}

sub _run_capture {
  my ( $command, $cwd ) = @_ ;
  my $quoted  = join q{ }, map { _shell_quote($_) } @{$command} ;
  my $old_cwd = abs_path(q{.}) ;
  chdir $cwd or die "Cannot chdir to '$cwd': $!\n" ;
  my $output = `$quoted 2>&1` ;
  my $status = $? >> 8 ;
  chdir $old_cwd or die "Cannot chdir to '$old_cwd': $!\n" ;
  die "$output\n" if $status != 0 ;
  return $output ;
}

sub _write_file {
  my ( $path, $content ) = @_ ;
  open my $fh, '>', $path
    or die "Cannot write '$path': $!\n" ;
  binmode $fh ;
  print {$fh} $content ;
  close $fh or die "Cannot close '$path': $!\n" ;
  return ;
}

sub _find_executable {
  my ($name) = @_ ;
  for my $dir ( split /:/, $ENV{PATH} // q{} ) {
    my $path = File::Spec->catfile( $dir, $name ) ;
    return $path if -x $path ;
  }
  return ;
}

sub _basename {
  my ($path) = @_ ;
  return ( File::Spec->splitpath($path) )[2] ;
}

sub _shell_quote {
  my ($value) = @_ ;
  $value =~ s/'/'"'"'/g ;
  return "'$value'" ;
}

sub _usage {
  my ($exit) = @_ ;
  my $stream = $exit == 0 ? *STDOUT : *STDERR ;
  print {$stream} <<'USAGE';
Usage:
  perl bench/compare-perl-go.pl [options]

Options:
  --assets PATH        Directory containing .jpg, .jpeg, or .png benchmark images
                       Default: assets
  --encode-iterations N
                       Encode and full iterations per image; default: 10
  --decode-iterations N
                       Decode iterations per image; default: 1000
  --iterations N      Backward-compatible override for both iteration counts
  --format FORMAT     Output format: text or markdown; default: text
  --include-full      Also benchmark file loading plus encoding
  --download-assets   Download the upstream SplatHash assets into --assets
  --help              Show this help text

The benchmark reports Perl encode and decode timings next to the pinned Go
reference implementation. Use --include-full for file-to-hash timing.
USAGE
  exit $exit ;
}
