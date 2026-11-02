"""
Scoring rule model for customizable duplicate scoring
"""

import enum

from sqlalchemy import (
    Boolean,
    Column,
    DateTime,
    Enum as SQLEnum,
    Index,
    Integer,
    Text,
)

from app.core.database import Base, utc_now


class RuleType(str, enum.Enum):
    """Type of scoring rule"""

    FILENAME_PATTERN = "filename_pattern"
    CODEC = "codec"
    RESOLUTION = "resolution"
    SOURCE = "source"
    BITRATE = "bitrate"
    AUDIO_CODEC = "audio_codec"
    CUSTOM = "custom"


class ScoringRule(Base):
    """User-defined scoring rules for duplicate detection"""

    __tablename__ = "scoring_rules"

    id = Column(Integer, primary_key=True, index=True)
    rule_type = Column(SQLEnum(RuleType), nullable=False, index=True)
    pattern = Column(Text, nullable=False)  # Regex pattern or value to match
    score_modifier = Column(
        Integer, default=0, nullable=False
    )  # Points to add/subtract
    enabled = Column(Boolean, default=True, nullable=False, index=True)
    created_at = Column(DateTime(timezone=True), default=utc_now, nullable=False)
    description = Column(Text, nullable=True)  # User-friendly description

    __table_args__ = (Index("idx_enabled_type", "enabled", "rule_type"),)

    def __repr__(self):
        return f"<ScoringRule(id={self.id}, type={self.rule_type}, pattern={self.pattern[:30]}, modifier={self.score_modifier})>"
