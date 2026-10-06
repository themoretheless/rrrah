use strict ;
use warnings ;

use ExtUtils::MakeMaker ;

WriteMakefile(
  NAME             => 'SplatHash',
  VERSION_FROM     => 'lib/SplatHash.pm',
  ABSTRACT         => 'Perl implementation of the SplatHash image placeholder algorithm',
  AUTHOR           => 'SplatHash contributors',
  LICENSE          => 'mit',
  MIN_PERL_VERSION => '5.020',
  EXE_FILES        => ['bin/splathash'],
  PREREQ_PM        => {
    'Cwd'          => 0,
    'Digest::SHA'  => 0,
    'Exporter'     => 0,
    'File::Path'   => 0,
    'File::Spec'   => 0,
    'File::Temp'   => 0,
    'Getopt::Long' => 0,
    'HTTP::Tiny'   => 0,
    'Imager'       => 0,
    'IPC::Open3'   => 0,
    'JSON::PP'     => 0,
    'Symbol'       => 0,
    'Test::More'   => 0,
    'Time::HiRes'  => 0,
  },
  META_MERGE => {
    'meta-spec' => { version => 2 },
    resources   => {
      repository => {
        type => 'git',
        url  => 'https://github.com/M-M-M-M/SplatHash-In-Perl.git',
        web  => 'https://github.com/M-M-M-M/SplatHash-In-Perl',
      },
      homepage   => 'https://github.com/M-M-M-M/SplatHash-In-Perl',
      bugtracker => {
        web => 'https://github.com/M-M-M-M/SplatHash-In-Perl/issues',
      },
    },
  },
) ;
