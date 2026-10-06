use strict ;
use warnings ;

use Test::More ;

my $project_license = _read_text('LICENSE') ;
like $project_license, qr/\AMIT License\n/,
  'project license uses the MIT text' ;
like $project_license, qr/Copyright \(c\) 2026 M-M-M-M/,
  'project license names M-M-M-M' ;

my $upstream_path    = 'LICENSES/third-party/SplatHash-MIT.txt' ;
my $upstream_license = _read_text($upstream_path) ;
like $upstream_license, qr/\AMIT License\n/,
  'upstream license uses the MIT text' ;
like $upstream_license, qr/Copyright \(c\) 2025 junevm/,
  'upstream copyright is preserved' ;

my $license_index = _read_text('LICENSES/README.md') ;
like $license_index, qr/\(third-party\/SplatHash-MIT\.txt\)/,
  'license index links to the upstream license' ;
like $license_index, qr/038b4f7025ec044ed9f319371e0ec2ed863a6f45/,
  'license index identifies the pinned upstream commit' ;

my $readme = _read_text('README.md') ;
like $readme, qr/\[MIT License\]\(LICENSE\)/,
  'README links to the project license' ;
like $readme, qr/\[third-party licences\]\(LICENSES\/README\.md\)/,
  'README links to the third-party licence index' ;
like $readme, qr/developed with assistance from OpenAI Codex/,
  'README discloses Codex assistance' ;
like $readme, qr/test-driven development workflow/,
  'README identifies the TDD workflow' ;
like $readme,
  qr/plans and major decisions were\s+reviewed by a human maintainer/i,
  'README describes the scope of human review' ;
like $readme, qr/not received a comprehensive line-by-line human review/,
  'README states the human review limitation' ;
like $readme,
  qr{\[perl-agents-md v1\.0\.0\]\(https://github\.com/M-M-M-M/perl-agents-md/releases/tag/v1\.0\.0\)},
  'README links to the validated perl-agents-md release' ;

my $assets = _read_text('ASSETS.md') ;
like $assets, qr/\Q$upstream_path\E/,
  'asset provenance links to the upstream license' ;

my $changelog = _read_text('CHANGELOG.md') ;
like $changelog, qr/^## Unreleased\n\n## 1\.0\.0 - 2026-06-09$/m,
  'changelog identifies the dated 1.0.0 release' ;

done_testing ;

sub _read_text {
  my ($path) = @_ ;
  open my $fh, '<:encoding(UTF-8)', $path
    or die "Cannot read '$path': $!" ;
  local $/ ;
  my $content = <$fh> ;
  close $fh or die "Cannot close '$path': $!" ;
  return $content ;
}
