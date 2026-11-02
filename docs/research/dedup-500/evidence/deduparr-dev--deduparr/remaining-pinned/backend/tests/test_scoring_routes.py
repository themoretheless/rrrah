"""
Tests for scoring rules API routes
"""

import pytest
from httpx import AsyncClient, ASGITransport

from app.main import app
from app.models.scoring_rule import ScoringRule, RuleType


@pytest.mark.asyncio
async def test_get_all_scoring_rules_empty(test_db):
    """Test getting all scoring rules when database is empty"""
    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get("/api/scoring/")
        assert response.status_code == 200
        assert response.json() == []


@pytest.mark.asyncio
async def test_create_scoring_rule(test_db):
    """Test creating a new scoring rule"""
    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.post(
            "/api/scoring/",
            json={
                "rule_type": "filename_pattern",
                "pattern": ".*BluRay.*",
                "score_modifier": 10,
                "enabled": True,
                "description": "Prefer BluRay releases",
            },
        )
        assert response.status_code == 201
        data = response.json()
        assert data["rule_type"] == "filename_pattern"
        assert data["pattern"] == ".*BluRay.*"
        assert data["score_modifier"] == 10
        assert data["enabled"] is True
        assert data["description"] == "Prefer BluRay releases"
        assert "id" in data


@pytest.mark.asyncio
async def test_get_scoring_rule_by_id(test_db):
    """Test getting a specific scoring rule by ID"""
    rule = ScoringRule(
        rule_type=RuleType.RESOLUTION,
        pattern="1080p",
        score_modifier=5,
        enabled=True,
        description="HD quality",
    )
    test_db.add(rule)
    await test_db.commit()
    await test_db.refresh(rule)

    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get(f"/api/scoring/{rule.id}")
        assert response.status_code == 200
        data = response.json()
        assert data["id"] == rule.id
        assert data["pattern"] == "1080p"
        assert data["score_modifier"] == 5


@pytest.mark.asyncio
async def test_get_scoring_rule_not_found(test_db):
    """Test getting a non-existent scoring rule returns 404"""
    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get("/api/scoring/999")
        assert response.status_code == 404


@pytest.mark.asyncio
async def test_update_scoring_rule(test_db):
    """Test updating an existing scoring rule"""
    rule = ScoringRule(
        rule_type=RuleType.CODEC,
        pattern="h264",
        score_modifier=0,
        enabled=True,
    )
    test_db.add(rule)
    await test_db.commit()
    await test_db.refresh(rule)

    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.put(
            f"/api/scoring/{rule.id}",
            json={
                "score_modifier": 15,
                "description": "Updated description",
            },
        )
        assert response.status_code == 200
        data = response.json()
        assert data["score_modifier"] == 15
        assert data["description"] == "Updated description"
        assert data["pattern"] == "h264"


@pytest.mark.asyncio
async def test_update_scoring_rule_not_found(test_db):
    """Test updating a non-existent scoring rule returns 404"""
    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.put(
            "/api/scoring/999",
            json={"score_modifier": 10},
        )
        assert response.status_code == 404


@pytest.mark.asyncio
async def test_delete_scoring_rule(test_db):
    """Test deleting a scoring rule"""
    rule = ScoringRule(
        rule_type=RuleType.SOURCE,
        pattern="WEB-DL",
        score_modifier=3,
        enabled=True,
    )
    test_db.add(rule)
    await test_db.commit()
    await test_db.refresh(rule)

    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.delete(f"/api/scoring/{rule.id}")
        assert response.status_code == 200
        data = response.json()
        assert data["status"] == "deleted"
        assert data["rule_id"] == rule.id

        response = await client.get(f"/api/scoring/{rule.id}")
        assert response.status_code == 404


@pytest.mark.asyncio
async def test_delete_scoring_rule_not_found(test_db):
    """Test deleting a non-existent scoring rule returns 404"""
    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.delete("/api/scoring/999")
        assert response.status_code == 404


@pytest.mark.asyncio
async def test_get_all_scoring_rules_with_data(test_db):
    """Test getting all scoring rules when data exists"""
    rules = [
        ScoringRule(
            rule_type=RuleType.FILENAME_PATTERN,
            pattern=".*BluRay.*",
            score_modifier=10,
            enabled=True,
        ),
        ScoringRule(
            rule_type=RuleType.RESOLUTION,
            pattern="4K",
            score_modifier=20,
            enabled=True,
        ),
        ScoringRule(
            rule_type=RuleType.CODEC,
            pattern="h265",
            score_modifier=15,
            enabled=False,
        ),
    ]
    for rule in rules:
        test_db.add(rule)
    await test_db.commit()

    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get("/api/scoring/")
        assert response.status_code == 200
        data = response.json()
        assert len(data) == 3


@pytest.mark.asyncio
async def test_get_scoring_rules_enabled_only(test_db):
    """Test getting only enabled scoring rules"""
    rules = [
        ScoringRule(
            rule_type=RuleType.RESOLUTION,
            pattern="1080p",
            score_modifier=5,
            enabled=True,
        ),
        ScoringRule(
            rule_type=RuleType.RESOLUTION,
            pattern="720p",
            score_modifier=3,
            enabled=False,
        ),
    ]
    for rule in rules:
        test_db.add(rule)
    await test_db.commit()

    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get("/api/scoring/?enabled_only=true")
        assert response.status_code == 200
        data = response.json()
        assert len(data) == 1
        assert data[0]["pattern"] == "1080p"


@pytest.mark.asyncio
async def test_get_rules_by_type(test_db):
    """Test getting scoring rules by type"""
    rules = [
        ScoringRule(
            rule_type=RuleType.RESOLUTION,
            pattern="1080p",
            score_modifier=5,
            enabled=True,
        ),
        ScoringRule(
            rule_type=RuleType.RESOLUTION,
            pattern="4K",
            score_modifier=10,
            enabled=True,
        ),
        ScoringRule(
            rule_type=RuleType.CODEC,
            pattern="h265",
            score_modifier=15,
            enabled=True,
        ),
    ]
    for rule in rules:
        test_db.add(rule)
    await test_db.commit()

    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get("/api/scoring/type/resolution")
        assert response.status_code == 200
        data = response.json()
        assert len(data) == 2
        assert all(r["rule_type"] == "resolution" for r in data)


@pytest.mark.asyncio
async def test_get_rules_by_type_enabled_only(test_db):
    """Test getting enabled scoring rules by type"""
    rules = [
        ScoringRule(
            rule_type=RuleType.CODEC,
            pattern="h264",
            score_modifier=5,
            enabled=True,
        ),
        ScoringRule(
            rule_type=RuleType.CODEC,
            pattern="h265",
            score_modifier=10,
            enabled=False,
        ),
    ]
    for rule in rules:
        test_db.add(rule)
    await test_db.commit()

    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.get("/api/scoring/type/codec?enabled_only=true")
        assert response.status_code == 200
        data = response.json()
        assert len(data) == 1
        assert data[0]["pattern"] == "h264"


@pytest.mark.asyncio
async def test_create_batch_scoring_rules(test_db):
    """Test creating multiple scoring rules at once"""
    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.post(
            "/api/scoring/batch",
            json=[
                {
                    "rule_type": "resolution",
                    "pattern": "1080p",
                    "score_modifier": 5,
                    "enabled": True,
                },
                {
                    "rule_type": "resolution",
                    "pattern": "4K",
                    "score_modifier": 10,
                    "enabled": True,
                },
                {
                    "rule_type": "codec",
                    "pattern": "h265",
                    "score_modifier": 15,
                    "enabled": True,
                },
            ],
        )
        assert response.status_code == 200
        data = response.json()
        assert len(data) == 3
        assert all("id" in rule for rule in data)

        response = await client.get("/api/scoring/")
        assert response.status_code == 200
        all_rules = response.json()
        assert len(all_rules) == 3


@pytest.mark.asyncio
async def test_update_scoring_rule_partial(test_db):
    """Test partial update of scoring rule"""
    rule = ScoringRule(
        rule_type=RuleType.SOURCE,
        pattern="BluRay",
        score_modifier=10,
        enabled=True,
        description="Original description",
    )
    test_db.add(rule)
    await test_db.commit()
    await test_db.refresh(rule)

    async with AsyncClient(
        transport=ASGITransport(app=app), base_url="http://test"
    ) as client:
        response = await client.put(
            f"/api/scoring/{rule.id}",
            json={"enabled": False},
        )
        assert response.status_code == 200
        data = response.json()
        assert data["enabled"] is False
        assert data["pattern"] == "BluRay"
        assert data["score_modifier"] == 10
        assert data["description"] == "Original description"
