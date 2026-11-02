"""
Scoring rules API endpoints for managing custom scoring rules
"""

from typing import List, Optional

from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel, ConfigDict
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_db
from app.models.scoring_rule import ScoringRule, RuleType

router = APIRouter()


class ScoringRuleCreate(BaseModel):
    """Request model for creating a scoring rule"""

    rule_type: RuleType
    pattern: str
    score_modifier: int
    enabled: bool = True
    description: Optional[str] = None


class ScoringRuleUpdate(BaseModel):
    """Request model for updating a scoring rule"""

    rule_type: Optional[RuleType] = None
    pattern: Optional[str] = None
    score_modifier: Optional[int] = None
    enabled: Optional[bool] = None
    description: Optional[str] = None


class ScoringRuleResponse(BaseModel):
    """Response model for scoring rules"""

    model_config = ConfigDict(from_attributes=True)

    id: int
    rule_type: RuleType
    pattern: str
    score_modifier: int
    enabled: bool
    description: Optional[str] = None


@router.get("/", response_model=List[ScoringRuleResponse])
async def get_all_scoring_rules(
    enabled_only: bool = False, db: AsyncSession = Depends(get_db)
):
    """
    Get all scoring rules

    Args:
        enabled_only: If True, only return enabled rules

    Returns:
        List of scoring rules
    """
    query = select(ScoringRule)
    if enabled_only:
        query = query.where(ScoringRule.enabled)

    query = query.order_by(ScoringRule.created_at.desc())

    result = await db.execute(query)
    rules = result.scalars().all()
    return rules


@router.get("/{rule_id}", response_model=ScoringRuleResponse)
async def get_scoring_rule(rule_id: int, db: AsyncSession = Depends(get_db)):
    """
    Get a specific scoring rule by ID

    Args:
        rule_id: ID of the scoring rule

    Returns:
        Scoring rule details
    """
    result = await db.execute(select(ScoringRule).where(ScoringRule.id == rule_id))
    rule = result.scalar_one_or_none()

    if not rule:
        raise HTTPException(status_code=404, detail=f"Scoring rule {rule_id} not found")

    return rule


@router.post("/", response_model=ScoringRuleResponse, status_code=201)
async def create_scoring_rule(
    rule_data: ScoringRuleCreate, db: AsyncSession = Depends(get_db)
):
    """
    Create a new scoring rule

    Args:
        rule_data: Scoring rule details

    Returns:
        Created scoring rule
    """
    rule = ScoringRule(
        rule_type=rule_data.rule_type,
        pattern=rule_data.pattern,
        score_modifier=rule_data.score_modifier,
        enabled=rule_data.enabled,
        description=rule_data.description,
    )

    db.add(rule)
    await db.commit()
    await db.refresh(rule)

    return rule


@router.put("/{rule_id}", response_model=ScoringRuleResponse)
async def update_scoring_rule(
    rule_id: int, rule_data: ScoringRuleUpdate, db: AsyncSession = Depends(get_db)
):
    """
    Update an existing scoring rule

    Args:
        rule_id: ID of the scoring rule
        rule_data: Updated scoring rule details

    Returns:
        Updated scoring rule
    """
    result = await db.execute(select(ScoringRule).where(ScoringRule.id == rule_id))
    rule = result.scalar_one_or_none()

    if not rule:
        raise HTTPException(status_code=404, detail=f"Scoring rule {rule_id} not found")

    if rule_data.rule_type is not None:
        rule.rule_type = rule_data.rule_type
    if rule_data.pattern is not None:
        rule.pattern = rule_data.pattern
    if rule_data.score_modifier is not None:
        rule.score_modifier = rule_data.score_modifier
    if rule_data.enabled is not None:
        rule.enabled = rule_data.enabled
    if rule_data.description is not None:
        rule.description = rule_data.description

    await db.commit()
    await db.refresh(rule)

    return rule


@router.delete("/{rule_id}")
async def delete_scoring_rule(rule_id: int, db: AsyncSession = Depends(get_db)):
    """
    Delete a scoring rule

    Args:
        rule_id: ID of the scoring rule

    Returns:
        Success message
    """
    result = await db.execute(select(ScoringRule).where(ScoringRule.id == rule_id))
    rule = result.scalar_one_or_none()

    if not rule:
        raise HTTPException(status_code=404, detail=f"Scoring rule {rule_id} not found")

    await db.delete(rule)
    await db.commit()

    return {"status": "deleted", "rule_id": rule_id}


@router.get("/type/{rule_type}", response_model=List[ScoringRuleResponse])
async def get_rules_by_type(
    rule_type: RuleType,
    enabled_only: bool = False,
    db: AsyncSession = Depends(get_db),
):
    """
    Get scoring rules by type

    Args:
        rule_type: Type of scoring rule
        enabled_only: If True, only return enabled rules

    Returns:
        List of scoring rules of the specified type
    """
    query = select(ScoringRule).where(ScoringRule.rule_type == rule_type)

    if enabled_only:
        query = query.where(ScoringRule.enabled)

    query = query.order_by(ScoringRule.created_at.desc())

    result = await db.execute(query)
    rules = result.scalars().all()
    return rules


@router.post("/batch", response_model=List[ScoringRuleResponse])
async def create_batch_scoring_rules(
    rules_data: List[ScoringRuleCreate], db: AsyncSession = Depends(get_db)
):
    """
    Create multiple scoring rules at once

    Args:
        rules_data: List of scoring rules to create

    Returns:
        List of created scoring rules
    """
    created_rules = []

    for rule_data in rules_data:
        rule = ScoringRule(
            rule_type=rule_data.rule_type,
            pattern=rule_data.pattern,
            score_modifier=rule_data.score_modifier,
            enabled=rule_data.enabled,
            description=rule_data.description,
        )
        db.add(rule)
        created_rules.append(rule)

    await db.commit()

    for rule in created_rules:
        await db.refresh(rule)

    return created_rules
