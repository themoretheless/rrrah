"""
Statistics API routes for dashboard
"""

from fastapi import APIRouter, Depends
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_db
from app.services.stats_service import (
    DashboardStats,
    DeletionActivity,
    RecentActivity,
    StatsService,
)

router = APIRouter()


@router.get("/dashboard", response_model=DashboardStats)
async def get_dashboard_stats(
    db: AsyncSession = Depends(get_db),
) -> DashboardStats:
    """Get comprehensive dashboard statistics"""
    service = StatsService(db)
    return await service.get_dashboard_stats()


@router.get("/recent-activity", response_model=list[RecentActivity])
async def get_recent_activity(
    limit: int = 10,
    db: AsyncSession = Depends(get_db),
) -> list[RecentActivity]:
    """Get recent duplicate detection activity"""
    service = StatsService(db)
    return await service.get_recent_activity(limit=limit)


@router.get("/recent-deletions", response_model=list[DeletionActivity])
async def get_recent_deletions(
    limit: int = 10,
    db: AsyncSession = Depends(get_db),
) -> list[DeletionActivity]:
    """Get recent deletion activity"""
    service = StatsService(db)
    return await service.get_recent_deletions(limit=limit)
