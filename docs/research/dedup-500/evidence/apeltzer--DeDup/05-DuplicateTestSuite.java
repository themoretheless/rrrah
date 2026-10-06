package test.java;

import org.junit.runner.RunWith;
import org.junit.runners.Suite;

@RunWith(Suite.class)
@Suite.SuiteClasses({
	AllReadsAsMergedTest.class,
	ForwardTest.class,
	ForwardWithMergedTest.class,
  MAPQZeroTest.class,
	ReverseTest.class,
  ReverseWithMergedTest.class,
	RMDupperTest.class,
	RMDupperUnsortedTest.class,
	SingleTest.class,
	StackTest.class,
	StackReadOneTest.class,
	StackReadTwoTest.class,
	StrandForwardTest.class,
	StrandReverseTest.class,
	YieldTest.class
})
public class DuplicateTestSuite {}
